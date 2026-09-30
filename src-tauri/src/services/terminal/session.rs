//! 终端会话：一个 PTY + shell 子进程 + 输出广播管线。
//!
//! 线程模型（每会话 3 线程，随会话终结自然退出）：
//! - reader：阻塞读 master 8KB → mpsc；
//! - broadcaster：合并窗口收束 → 增量 UTF-8 解码 → scrollback + 订阅广播；EOF 冲刷并发 Exit；
//! - waiter：阻塞 wait 记录退出码。
//!
//! 锁序约定：subscribers → scrollback（attach 与 broadcaster 同序，回放无间隙且不重复）。

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{MasterPty, PtySize};
use tauri::ipc::Channel;
use uuid::Uuid;

use crate::domain::{TerminalEvent, TerminalEventKind, TerminalInfo};
use crate::errors::AppError;
use crate::services::scripts::runner;
use crate::services::terminal::shell;
use crate::services::terminal::utf8::Utf8Accumulator;

/// reader 单次读取块。
const READ_CHUNK: usize = 8 * 1024;
/// 合并窗口内单条消息字节上限（超出即切分，防单帧过大）。
const MESSAGE_CAP_BYTES: usize = 64 * 1024;
/// 广播合并等待（读端到齐再发，削峰 IPC 帧数）。
const COALESCE_WINDOW: Duration = Duration::from_millis(8);
/// scrollback 回放字节预算。
pub const SCROLLBACK_BUDGET_BYTES: usize = 256 * 1024;
/// init command 延迟写入（PowerShell 启动/Profile 加载期立即写入可能被吞）。
const INIT_DELAY: Duration = Duration::from_millis(400);

/// 字节预算环形回放缓冲（attach 时快照重放）。
pub(crate) struct Scrollback {
    budget: usize,
    chunks: VecDeque<String>,
    len: usize,
}

impl Scrollback {
    fn new(budget: usize) -> Self {
        Scrollback {
            budget,
            chunks: VecDeque::new(),
            len: 0,
        }
    }

    fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.len += text.len();
        self.chunks.push_back(text.to_string());
        // 淘汰最旧直至入预算；末段永不被淘汰（单段 ≤ 消息上限 < 预算）
        while self.len > self.budget && self.chunks.len() > 1 {
            if let Some(front) = self.chunks.pop_front() {
                self.len -= front.len();
            }
        }
    }

    fn snapshot(&self) -> String {
        let mut out = String::with_capacity(self.len);
        for chunk in &self.chunks {
            out.push_str(chunk);
        }
        out
    }
}

struct Subscriber {
    attach_id: String,
    channel: Channel<TerminalEvent>,
}

/// 会话静态元信息（exited/exit_code 为原子态，经 snapshot 合成 TerminalInfo）。
struct SessionMeta {
    shell: String,
    cwd: String,
    venv: Option<String>,
    title: Option<String>,
    created_at: String,
}

pub struct TerminalSession {
    pub id: String,
    meta: SessionMeta,
    /// dispose 后置 None（强制关闭 PTY——reader 必得 EOF，线程必然退出）
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    scrollback: Mutex<Scrollback>,
    subscribers: Mutex<Vec<Subscriber>>,
    child_pid: Option<u32>,
    exit_code: Mutex<Option<i32>>,
    exited: AtomicBool,
}

/// spawn 所需的全部已解析配置（由 registry 组装）。
pub struct SessionSpawn {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env_overlay: std::collections::HashMap<String, String>,
    pub cols: u16,
    pub rows: u16,
    pub shell_label: String,
    pub venv_label: Option<String>,
    pub title: Option<String>,
    pub init_commands: Vec<String>,
}

impl TerminalSession {
    /// 打开 PTY、spawn shell、启动三线程。阻塞（openpty/spawn 数毫秒级）。
    pub fn spawn(cfg: SessionSpawn) -> Result<Arc<Self>, AppError> {
        let pty_system = portable_pty::native_pty_system();
        let size = PtySize {
            rows: cfg.rows,
            cols: cfg.cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size).map_err(|e| AppError::TerminalSpawn {
            message: format!("open pty: {e}"),
        })?;
        let cmd = super::shell::build_command(
            &shell::ShellSpawn {
                program: cfg.program.clone(),
                args: cfg.args.clone(),
            },
            &cfg.cwd,
            &cfg.env_overlay,
        );
        let mut child = pair.slave.spawn_command(cmd).map_err(|e| {
            AppError::TerminalSpawn {
                message: format!("spawn shell {}: {e}", cfg.program.display()),
            }
        })?;
        let child_pid = child.process_id();
        let master = pair.master;
        let reader = master.try_clone_reader().map_err(|e| AppError::TerminalSpawn {
            message: format!("clone pty reader: {e}"),
        })?;
        let writer = master.take_writer().map_err(|e| AppError::TerminalSpawn {
            message: format!("take pty writer: {e}"),
        })?;
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();

        let session = Arc::new(Self {
            id: Uuid::new_v4().to_string(),
            meta: SessionMeta {
                shell: cfg.shell_label,
                cwd: cfg.cwd.to_string_lossy().into_owned(),
                venv: cfg.venv_label,
                title: cfg.title,
                created_at,
            },
            master: Mutex::new(Some(master)),
            writer: Mutex::new(Some(writer)),
            scrollback: Mutex::new(Scrollback::new(SCROLLBACK_BUDGET_BYTES)),
            subscribers: Mutex::new(Vec::new()),
            child_pid,
            exit_code: Mutex::new(None),
            exited: AtomicBool::new(false),
        });

        // waiter：阻塞 wait 记录退出码（持有 child 所有权）
        {
            let waiter_session = Arc::clone(&session);
            std::thread::spawn(move || {
                if let Ok(status) = child.wait() {
                    if let Ok(mut code) = waiter_session.exit_code.lock() {
                        *code = Some(status.exit_code() as i32);
                    }
                }
                waiter_session.exited.store(true, Ordering::Release);
            });
        }

        // reader → broadcaster 管线
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        {
            std::thread::spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; READ_CHUNK];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break, // EOF（Err 视作管道破裂）
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break; // broadcaster 已退出
                            }
                        }
                    }
                }
                // drop(tx) 随闭包结束触发 broadcaster Disconnected
            });
        }
        {
            let session = Arc::clone(&session);
            std::thread::spawn(move || session.broadcast_loop(rx));
        }

        // init commands：延迟写入（shell 启动期立即写入可能被吞）
        if !cfg.init_commands.is_empty() {
            let session = Arc::clone(&session);
            std::thread::spawn(move || {
                std::thread::sleep(INIT_DELAY);
                for command in &cfg.init_commands {
                    if session.write(&format!("{command}\r")).is_err() {
                        break; // 会话已终结
                    }
                    // 命令间稍隔，避免 PowerShell 串行解析丢行
                    std::thread::sleep(Duration::from_millis(60));
                }
            });
        }

        tracing::info!(
            "[terminal] session {} spawned: shell={} venv={:?} pid={:?}",
            session.id,
            session.meta.shell,
            session.meta.venv,
            session.child_pid
        );
        Ok(session)
    }

    pub fn write(&self, data: &str) -> Result<(), AppError> {
        let mut guard = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        let Some(writer) = guard.as_mut() else {
            return Err(AppError::TerminalIo {
                message: "session closed".into(),
            });
        };
        writer
            .write_all(data.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|e| AppError::TerminalIo {
                message: format!("pty write: {e}"),
            })
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), AppError> {
        let mut guard = self.master.lock().unwrap_or_else(|e| e.into_inner());
        let Some(master) = guard.as_mut() else {
            return Err(AppError::TerminalIo {
                message: "session closed".into(),
            });
        };
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::TerminalIo {
                message: format!("pty resize: {e}"),
            })
    }

    /// 附着：持订阅锁快照回放 → 注册订阅（与 broadcaster 广播互斥，无间隙且不重复）。
    /// 会话已退出时补发 Exit，视图即刻进入终态。
    pub fn attach(&self, channel: Channel<TerminalEvent>) -> String {
        let attach_id = Uuid::new_v4().to_string();
        let mut subs = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        let replay = self
            .scrollback
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot();
        let _ = channel.send(TerminalEvent {
            kind: TerminalEventKind::Replay,
            data: replay,
            exit_code: None,
        });
        if self.exited.load(Ordering::Acquire) {
            let _ = channel.send(TerminalEvent {
                kind: TerminalEventKind::Exit,
                data: String::new(),
                exit_code: self.current_exit_code(),
            });
        }
        subs.push(Subscriber {
            attach_id: attach_id.clone(),
            channel,
        });
        attach_id
    }

    pub fn detach(&self, attach_id: &str) {
        self.subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|sub| sub.attach_id != attach_id);
    }

    fn current_exit_code(&self) -> Option<i32> {
        *self.exit_code.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 销毁：杀进程树 + **立即关闭 PTY**（take 置 None 触发 drop → ClosePseudoConsole）
    /// ——reader 必得 EOF、broadcaster/waiter 线程必然退出，根治孙进程幸存时
    /// 「reader 无 EOF → 线程常驻 → 句柄泄漏」路径。幂等（take None 为 no-op）。
    pub fn dispose(&self) {
        runner::kill_tree(self.child_pid);
        if let Ok(mut guard) = self.master.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.writer.lock() {
            *guard = None;
        }
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn snapshot(&self) -> TerminalInfo {
        TerminalInfo {
            id: self.id.clone(),
            shell: self.meta.shell.clone(),
            cwd: self.meta.cwd.clone(),
            venv: self.meta.venv.clone(),
            title: self.meta.title.clone(),
            created_at: self.meta.created_at.clone(),
            exited: self.is_exited(),
            exit_code: self.current_exit_code(),
        }
    }

    /// 广播循环：合并窗口收束 → 解码 → scrollback + 订阅分发；EOF 后发 Exit。
    fn broadcast_loop(self: Arc<Self>, rx: Receiver<Vec<u8>>) {
        let mut acc = Utf8Accumulator::new();
        let mut raw: Vec<u8> = Vec::new();
        loop {
            match rx.recv_timeout(COALESCE_WINDOW) {
                Ok(bytes) => {
                    raw.extend_from_slice(&bytes);
                    // 窗口内到齐即收（相邻 chunk 合并成单帧）
                    loop {
                        match rx.try_recv() {
                            Ok(more) => {
                                raw.extend_from_slice(&more);
                                if raw.len() >= MESSAGE_CAP_BYTES {
                                    self.flush(&mut acc, &mut raw);
                                }
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                self.flush(&mut acc, &mut raw);
                                self.broadcast_exit();
                                return;
                            }
                        }
                    }
                    self.flush(&mut acc, &mut raw);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !raw.is_empty() {
                        self.flush(&mut acc, &mut raw);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.flush(&mut acc, &mut raw);
                    self.broadcast_exit();
                    return;
                }
            }
        }
    }

    fn flush(&self, acc: &mut Utf8Accumulator, raw: &mut Vec<u8>) {
        if raw.is_empty() {
            return;
        }
        let text = acc.push(raw);
        raw.clear();
        if text.is_empty() {
            return;
        }
        // 锁序：subscribers → scrollback（与 attach 一致）
        let mut subs = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        self.scrollback
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(&text);
        let event = TerminalEvent {
            kind: TerminalEventKind::Output,
            data: text,
            exit_code: None,
        };
        // 发送失败（webview 已销毁）即剪枝该订阅
        subs.retain(|sub| sub.channel.send(event.clone()).is_ok());
    }

    fn broadcast_exit(&self) {
        // reader EOF 与 waiter 记录退出码并发——小窗口等 exited 置位
        // （waiter 先写 code 再置 exited，故 exited 后 code 为终值；≤500ms）
        let mut exit_code = self.current_exit_code();
        if exit_code.is_none() {
            for _ in 0..10 {
                std::thread::sleep(Duration::from_millis(50));
                if self.exited.load(Ordering::Acquire) {
                    exit_code = self.current_exit_code();
                    break;
                }
            }
        }
        let event = TerminalEvent {
            kind: TerminalEventKind::Exit,
            data: String::new(),
            exit_code,
        };
        self.subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|sub| sub.channel.send(event.clone()).is_ok());
        tracing::info!(
            "[terminal] session {} exited: code={:?}",
            self.id,
            event.exit_code
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sb() -> Scrollback {
        Scrollback::new(1024)
    }

    #[test]
    fn scrollback_snapshot_concatenates() {
        let mut s = sb();
        s.push("hello ");
        s.push("world");
        assert_eq!(s.snapshot(), "hello world");
    }

    #[test]
    fn scrollback_evicts_oldest_over_budget() {
        let mut s = sb();
        let chunk = "x".repeat(400);
        for _ in 0..5 {
            s.push(&chunk);
        }
        // 5×400 = 2000 > 1024 预算：仅保留尾部两段
        assert!(s.len <= 1024 + chunk.len());
        assert_eq!(s.snapshot().len(), 800);
    }

    #[test]
    fn scrollback_ignores_empty() {
        let mut s = sb();
        s.push("");
        assert_eq!(s.snapshot(), "");
        assert_eq!(s.len, 0);
    }
}
