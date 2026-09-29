//! 脚本执行内核：机制（流式截断/取消令牌/杀树/运行注册表），不含单飞等策略。

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::errors::AppError;

/// 单流 head 区上限（前 768KiB 原样保留）。
pub const OUTPUT_HEAD_CAP_BYTES: usize = 768 * 1024;
/// 单流 tail 区上限（末 256KiB；traceback 多在末尾，纯头截断会丢最有价值部分）。
pub const OUTPUT_TAIL_CAP_BYTES: usize = 256 * 1024;
/// reader 读取块大小。
const READ_CHUNK: usize = 8 * 1024;
/// kill 后 wait 的防御性兜底（TerminateProcess/SIGKILL 后应立即返回）。
const KILL_GRACE: Duration = Duration::from_secs(10);
/// reader 汇合兜底（极端情况：孙进程继承管道句柄不退出）。
const READER_GRACE: Duration = Duration::from_secs(5);

/// spawn 选项。
pub struct SpawnOptions {
    pub interpreter: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    /// 写入子进程 stdin 的 JSON 文档（`passAs=stdin` 参数的类型化投影）。
    pub stdin_json: Value,
    /// 运行超时（机制参数化；300s 硬编码属 service 层策略）。
    pub timeout: Duration,
}

/// 运行结果（IPC 无关的内核形态；由 service 组装为 ScriptRunResult）。
/// 超时信号内嵌于 stderr note（与基线行为一致），不单独暴露字段。
#[derive(Debug)]
pub struct RunOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub cancelled: bool,
}

/// 运行注册表：run_id → 取消令牌。
/// check+insert 在同一锁临界区内原子完成（单飞语义）；不跨 await 持锁。
#[derive(Default)]
pub struct ScriptRunRegistry {
    runs: Mutex<HashMap<Uuid, CancellationToken>>,
}

impl ScriptRunRegistry {
    pub fn global() -> &'static ScriptRunRegistry {
        static REGISTRY: OnceLock<ScriptRunRegistry> = OnceLock::new();
        REGISTRY.get_or_init(ScriptRunRegistry::default)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, CancellationToken>> {
        // 锁中毒恢复：一次运行中的 panic 不应永久破坏取消能力
        self.runs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 原子注册：当前无活动运行则登记并返回该 run 的取消令牌；已有运行返回 None。
    pub fn try_register(&self, run_id: Uuid) -> Option<CancellationToken> {
        let token = CancellationToken::new();
        let mut runs = self.lock();
        if !runs.is_empty() {
            return None;
        }
        runs.insert(run_id, token.clone());
        Some(token)
    }

    pub fn unregister(&self, run_id: &Uuid) {
        self.lock().remove(run_id);
    }

    /// 取消全部活动运行（单飞语义下至多一个）。
    pub fn cancel_all(&self) {
        for token in self.lock().values() {
            token.cancel();
        }
    }
}

/// 流式执行子进程并收集带上限的输出。
///
/// - stdout/stderr 各由独立 reader 任务并发读取（串行会因 pipe 写满死锁）；
/// - 超限后继续读但丢弃，直到 EOF（停止读会让子进程永久阻塞在 write 上）；
/// - reader 不感知取消：取消经 token → 杀进程 → 写端关闭 → reader 读到 EOF 自然结束；
/// - stdin JSON 由独立任务写入（大参数正是 stdin 通道用途，不能阻塞主流程）。
pub async fn spawn_and_stream(
    opts: SpawnOptions,
    token: CancellationToken,
) -> Result<RunOutcome, AppError> {
    // pre-spawn 取消：直接返回空结果，不进杀进程路径
    if token.is_cancelled() {
        return Ok(RunOutcome {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            cancelled: true,
        });
    }

    let mut cmd = Command::new(&opts.interpreter);
    cmd.args(&opts.args)
        .current_dir(&opts.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().map_err(|e| AppError::InternalError {
        message: format!("failed to start python ({}): {e}", opts.interpreter),
    })?;

    spawn_stdin_writer(&mut child, &opts.stdin_json);

    let stdout_task = child.stdout.take().map(|p| {
        tokio::spawn(read_capped(
            p,
            LimitedOutput::new(OUTPUT_HEAD_CAP_BYTES, OUTPUT_TAIL_CAP_BYTES),
        ))
    });
    let stderr_task = child.stderr.take().map(|p| {
        tokio::spawn(read_capped(
            p,
            LimitedOutput::new(OUTPUT_HEAD_CAP_BYTES, OUTPUT_TAIL_CAP_BYTES),
        ))
    });

    let pid = child.id();
    let wait_fut = child.wait();
    tokio::pin!(wait_fut);

    let (status, cancelled, timed_out) = tokio::select! {
        res = &mut wait_fut => {
            let status = res.map_err(|e| AppError::InternalError {
                message: format!("wait script process: {e}"),
            })?;
            // 自然完成与取消同时到达时以取消语义呈现（与旧实现一致）
            (status, token.is_cancelled(), false)
        }
        _ = token.cancelled() => {
            kill_tree(pid);
            (wait_after_kill(&mut wait_fut).await?, true, false)
        }
        _ = tokio::time::sleep(opts.timeout) => {
            kill_tree(pid);
            (wait_after_kill(&mut wait_fut).await?, false, true)
        }
    };

    let stdout = match stdout_task {
        Some(t) => join_limited(t).await.finish(),
        None => String::new(),
    };
    let mut stderr = match stderr_task {
        Some(t) => join_limited(t).await.finish(),
        None => String::new(),
    };
    if timed_out {
        let note = format!(
            "\n\n[scripts] run timed out after {}s and was killed",
            opts.timeout.as_secs()
        );
        if stderr.is_empty() {
            stderr = note.trim_start().to_string();
        } else {
            stderr.push_str(&note);
        }
    }

    Ok(RunOutcome {
        exit_code: status.code(),
        stdout,
        stderr,
        cancelled,
    })
}

/// stdin 写入任务：写完 JSON 后 drop 关闭管道（脚本侧 `json.load(sys.stdin)` 返回）。
/// 独立任务避免大 JSON 阻塞主流程；脚本被杀后写端失败，任务自然结束。
fn spawn_stdin_writer(child: &mut Child, stdin_json: &Value) {
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(stdin_json).unwrap_or_else(|_| b"{}".to_vec());
        tokio::spawn(async move {
            let _ = stdin.write_all(&payload).await;
        });
    }
}

async fn read_capped<R: AsyncRead + Unpin>(mut reader: R, mut cap: LimitedOutput) -> LimitedOutput {
    let mut buf = vec![0u8; READ_CHUNK];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => cap.push(&buf[..n]),
            Err(e) => {
                // 取消/杀进程导致的管道错误：保留已读内容
                tracing::debug!("script output read stopped: {e}");
                break;
            }
        }
    }
    cap
}

async fn wait_after_kill<F>(fut: F) -> Result<std::process::ExitStatus, AppError>
where
    F: Future<Output = std::io::Result<std::process::ExitStatus>>,
{
    match tokio::time::timeout(KILL_GRACE, fut).await {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(e)) => Err(AppError::InternalError {
            message: format!("wait script after kill: {e}"),
        }),
        Err(_) => Err(AppError::InternalError {
            message: "script process did not exit after kill".into(),
        }),
    }
}

async fn join_limited(task: JoinHandle<LimitedOutput>) -> LimitedOutput {
    let abort = task.abort_handle();
    match tokio::time::timeout(READER_GRACE, task).await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            // join 失败（任务 panic/取消）：放弃该流残余
            tracing::warn!("script output reader failed: {e}");
            LimitedOutput::new(0, 0)
        }
        Err(_) => {
            // 防御：孙进程持有管道不退出；中止只丢数据不泄漏资源
            tracing::warn!("script output reader grace timeout, aborting");
            abort.abort();
            LimitedOutput::new(0, 0)
        }
    }
}

/// 杀进程树：Windows `taskkill /T /F`（同步调用，自用单飞场景取舍接受）；
/// Unix SIGKILL（TERM 可被忽略）。快照后新生的孙进程存在枚举竞态，
/// `kill_on_drop` 作为最后保底（只杀直接子进程）。
pub fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
}

/// 带上限的输出累积器：head 区（前 head_cap 字节）+ tail 区（末 tail_cap 字节，
/// VecDeque 按总字节数淘汰最旧）。总量恒 ≤ head_cap + tail_cap。
pub struct LimitedOutput {
    head_cap: usize,
    tail_cap: usize,
    head: Vec<u8>,
    tail: VecDeque<Vec<u8>>,
    tail_len: usize,
    total: usize,
    dropped_middle: bool,
}

impl LimitedOutput {
    pub fn new(head_cap: usize, tail_cap: usize) -> Self {
        LimitedOutput {
            head_cap,
            tail_cap,
            head: Vec::new(),
            tail: VecDeque::new(),
            tail_len: 0,
            total: 0,
            dropped_middle: false,
        }
    }

    /// 累积一段输出；超限后继续计入 total 但中段被丢弃。
    pub fn push(&mut self, bytes: &[u8]) {
        self.total += bytes.len();
        if self.head.len() < self.head_cap {
            let take = bytes.len().min(self.head_cap - self.head.len());
            self.head.extend_from_slice(&bytes[..take]);
            self.push_tail(&bytes[take..]);
        } else {
            self.dropped_middle = true;
            self.push_tail(bytes);
        }
    }

    fn push_tail(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.dropped_middle = true;
        // 单段超过 tail_cap 时只保留末尾
        let bytes = if bytes.len() > self.tail_cap {
            &bytes[bytes.len() - self.tail_cap..]
        } else {
            bytes
        };
        self.tail.push_back(bytes.to_vec());
        self.tail_len += bytes.len();
        while self.tail_len > self.tail_cap {
            let front_len = self.tail.front().map(Vec::len).unwrap_or(0);
            if front_len <= self.tail_len - self.tail_cap {
                let front = self.tail.pop_front();
                self.tail_len -= front_len;
                debug_assert!(front.is_some());
            } else {
                // 需要劈开最旧段
                let excess = self.tail_len - self.tail_cap;
                if let Some(front) = self.tail.front_mut() {
                    front.drain(..excess);
                }
                self.tail_len -= excess;
                break;
            }
        }
    }

    /// 汇出最终文本：head + tail 拼接后经 `decode_output` 解码；
    /// tail 前导残缺字节（UTF-8 多字节字符劈开）先丢弃，接缝处至多一个 U+FFFD
    /// （对 GB18030 字节流，该 skip 可能多丢 ≤3 字节——接缝字符本就在丢弃语义内）。
    pub fn finish(self) -> String {
        let mut buf = Vec::with_capacity(self.head.len() + self.tail_len);
        buf.extend_from_slice(&self.head);
        let mut tail: Vec<u8> = Vec::with_capacity(self.tail_len);
        for chunk in &self.tail {
            tail.extend_from_slice(chunk);
        }
        let skip = tail
            .iter()
            .take(3)
            .take_while(|b| *b & 0xC0 == 0x80)
            .count();
        buf.extend_from_slice(&tail[skip..]);

        let kept = buf.len();
        let text = decode_output(&buf);
        if self.total > kept {
            format!(
                "{text}\n\n… [truncated, total {} bytes, kept {kept} bytes]",
                self.total
            )
        } else {
            text
        }
    }
}

/// 子进程输出解码：UTF-8 优先，非 UTF-8 回退 GB18030（GBK/GB2312 超集，ASCII 透明）。
///
/// 中文 Windows 上 Python <3.15 的管道 stdio 编码是 cp936（PEP 686 前的默认），
/// GBK 字节经 `from_utf8_lossy` 会变 U+FFFD 混杂偶发有效对的乱码。
/// 已知限制：混合编码流（GBK 输出混入子进程 UTF-8 原始字节）会被整体按
/// GB18030 解码，UTF-8 段因此损坏——个人工具不做逐段探测。
fn decode_output(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(_) => encoding_rs::GB18030.decode(bytes).0.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limited_output_short_passthrough() {
        let mut o = LimitedOutput::new(100, 50);
        o.push(b"hello");
        assert_eq!(o.finish(), "hello");
    }

    #[test]
    fn limited_output_keeps_head_and_tail_drops_middle() {
        let mut o = LimitedOutput::new(10, 10);
        // head: "0123456789"（10B），随后 10B 中段被丢弃，tail 保留最后 10B
        o.push(b"0123456789");
        o.push(b"AAAAAAAAAA");
        o.push(b"BBBBBBBBBB");
        let text = o.finish();
        assert!(text.starts_with("0123456789"));
        assert!(text.contains("BBBBBBBBBB"));
        assert!(!text.contains('A'));
        assert!(text.contains("[truncated, total 30 bytes, kept 20 bytes]"));
    }

    #[test]
    fn limited_output_single_oversize_push_keeps_end() {
        let mut o = LimitedOutput::new(4, 4);
        o.push(b"0123456789"); // head 4B + tail 末 4B
        let text = o.finish();
        assert!(text.starts_with("0123"));
        assert!(text.ends_with("6789\n\n… [truncated, total 10 bytes, kept 8 bytes]"));
    }

    #[test]
    fn limited_output_utf8_seam_bounded_artifact() {
        // "界" 是 3 字节 (E7 95 8C)；head 5B 劈开它，tail 以完整字符开始
        let mut o = LimitedOutput::new(5, 16);
        o.push("abc\u{754c}defgh".as_bytes()); // abc + 界(3B) + defgh
        let text = o.finish();
        // 接缝伪影限于单字符：UTF-8 流的 dangling E7 95 在 GB18030 回退下
        // 解成一个 GBK 字符（或 U+FFFD）——均不产生双重 FFFD
        assert!(text.starts_with("abc"), "text was: {text}");
        assert!(text.contains("defgh"), "text was: {text}");
        assert!(!text.contains("\u{FFFD}\u{FFFD}"));
    }

    #[test]
    fn limited_output_decodes_gbk_fallback() {
        // 中文 Windows Python <3.15 管道默认编码 cp936/GBK
        let (gbk, _, _) = encoding_rs::GB18030.encode("用户：222\n创建文件：output.txt\n");
        let mut o = LimitedOutput::new(4096, 1024);
        o.push(&gbk);
        let text = o.finish();
        assert!(text.contains("用户：222"), "text was: {text}");
        assert!(text.contains("创建文件：output.txt"), "text was: {text}");
    }

    #[test]
    fn limited_output_valid_utf8_passthrough() {
        let mut o = LimitedOutput::new(4096, 1024);
        o.push("用户：222".as_bytes());
        assert_eq!(o.finish(), "用户：222");
    }

    #[test]
    fn limited_output_gbk_truncated_still_decodes() {
        let volume = "用户名测试行\n".repeat(64);
        let (gbk, _, _) = encoding_rs::GB18030.encode(&volume);
        let mut o = LimitedOutput::new(32, 32); // 极小上限触发截断
        o.push(&gbk);
        let text = o.finish();
        assert!(
            text.contains("[truncated, total"),
            "text tail: {}",
            &text[text.len().saturating_sub(120)..]
        );
        // 截断后残余仍是可读中文（接缝至多个别 U+FFFD，不出现 û/ļ 式混杂乱码）
        assert!(text.contains("用户名"), "text was: {text}");
        assert!(!text.contains('û') && !text.contains('ļ'));
    }

    #[test]
    fn registry_single_flight_and_cancel() {
        let reg = ScriptRunRegistry::default();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let t1 = reg.try_register(id1).expect("first register");
        assert!(reg.try_register(id2).is_none(), "second register rejected");
        assert!(!t1.is_cancelled());

        reg.cancel_all();
        assert!(t1.is_cancelled());

        reg.unregister(&id1);
        let t2 = reg.try_register(id2).expect("register after unregister");
        assert!(!t2.is_cancelled());
    }

    // ---- 集成测试（跨平台命令） ----

    fn base_opts(interpreter: &str, args: &[&str], timeout: Duration) -> SpawnOptions {
        // 继承进程 env（与生产一致：service 侧 env 由进程 env 合并而来）。
        // cmd 解析 ping/findstr 等外部命令依赖 PATH，清空 env 会找不到可执行文件。
        let env: HashMap<String, String> = std::env::vars().collect();
        SpawnOptions {
            interpreter: interpreter.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: std::env::temp_dir(),
            env,
            stdin_json: serde_json::json!({}),
            timeout,
        }
    }

    fn echo_cmd() -> (&'static str, Vec<&'static str>) {
        if cfg!(windows) {
            ("cmd", vec!["/c", "echo runner_ok"])
        } else {
            ("sh", vec!["-c", "echo runner_ok"])
        }
    }

    fn sleep_cmd() -> (&'static str, Vec<&'static str>) {
        if cfg!(windows) {
            ("cmd", vec!["/c", "ping -n 60 127.0.0.1"])
        } else {
            ("sleep", vec!["60"])
        }
    }

    fn stdin_echo_cmd() -> (&'static str, Vec<&'static str>) {
        if cfg!(windows) {
            ("cmd", vec!["/c", "findstr .*"])
        } else {
            ("cat", vec![])
        }
    }

    #[tokio::test]
    async fn spawns_and_collects_output() {
        let (prog, args) = echo_cmd();
        let out = spawn_and_stream(
            base_opts(prog, &args, Duration::from_secs(30)),
            CancellationToken::new(),
        )
        .await
        .expect("run echo");
        assert_eq!(out.exit_code, Some(0));
        assert!(out.stdout.contains("runner_ok"));
        assert!(!out.cancelled);
    }

    #[tokio::test]
    async fn cancel_before_spawn_returns_empty_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        let out = spawn_and_stream(
            base_opts("definitely-not-a-program", &[], Duration::from_secs(5)),
            token,
        )
        .await
        .expect("cancelled before spawn");
        assert!(out.cancelled);
        assert_eq!(out.exit_code, None);
        assert!(out.stdout.is_empty());
    }

    #[tokio::test]
    async fn cancellation_kills_long_process() {
        let (prog, args) = sleep_cmd();
        let token = CancellationToken::new();
        let t = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            t.cancel();
        });
        let started = std::time::Instant::now();
        let out = spawn_and_stream(base_opts(prog, &args, Duration::from_secs(60)), token)
            .await
            .expect("cancel sleep");
        assert!(out.cancelled);
        assert!(started.elapsed() < Duration::from_secs(15));
    }

    #[tokio::test]
    async fn timeout_kills_and_notes() {
        let (prog, args) = sleep_cmd();
        let out = spawn_and_stream(
            base_opts(prog, &args, Duration::from_millis(300)),
            CancellationToken::new(),
        )
        .await
        .expect("timeout sleep");
        assert!(!out.cancelled);
        assert!(out.stderr.contains("timed out"));
    }

    #[tokio::test]
    async fn stdin_json_reaches_child_typed() {
        let (prog, args) = stdin_echo_cmd();
        let mut opts = base_opts(prog, &args, Duration::from_secs(30));
        opts.stdin_json = serde_json::json!({"n": 10, "flag": false, "s": "hi"});
        let out = spawn_and_stream(opts, CancellationToken::new())
            .await
            .expect("stdin roundtrip");
        // 整数优先：10 而非 10.0
        assert!(
            out.stdout.contains("\"n\":10"),
            "stdout was: {}",
            out.stdout
        );
        assert!(out.stdout.contains("\"flag\":false"));
        assert!(out.stdout.contains("\"s\":\"hi\""));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn volume_output_is_truncated_not_buffered() {
        // ~30000 * 41B ≈ 1.2MB：超过 head+tail 的 1MiB，验证截断标记
        let out = spawn_and_stream(
            base_opts(
                "cmd",
                &[
                    "/c",
                    "for /L %i in (1,1,30000) do @echo aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ],
                Duration::from_secs(60),
            ),
            CancellationToken::new(),
        )
        .await
        .expect("volume run");
        assert_eq!(out.exit_code, Some(0));
        assert!(
            out.stdout.contains("[truncated, total"),
            "stdout tail: {}",
            &out.stdout[out.stdout.len().saturating_sub(200)..]
        );
    }
}
