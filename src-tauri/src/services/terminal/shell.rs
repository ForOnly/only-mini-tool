//! Shell 探测与 spawn 配置（终端专用）。
//!
//! 与 runner.rs 的管道执行刻意相反：终端全量继承系统环境 + 追加覆盖
//! （venv 激活即 env 注入，不执行任何 Activate.ps1/activate.bat——
//! 绕开 ExecutionPolicy 与路径转义两类坑）。

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellKind {
    Pwsh,
    Powershell,
    Cmd,
    Custom(PathBuf),
}

impl ShellKind {
    /// 展示/记录用标签。
    pub fn label(&self) -> String {
        match self {
            Self::Pwsh => "pwsh".into(),
            Self::Powershell => "powershell".into(),
            Self::Cmd => "cmd".into(),
            Self::Custom(path) => path.to_string_lossy().into_owned(),
        }
    }
}

/// 自动探测：PATH 手工扫描 pwsh → powershell → cmd
/// （不 spawn where.exe——GUI 进程 spawn 会闪控制台窗）。
#[cfg(windows)]
pub fn detect_shell() -> ShellKind {
    let Some(path_var) = std::env::var_os("PATH") else {
        return ShellKind::Cmd;
    };
    for dir in std::env::split_paths(&path_var) {
        if dir.join("pwsh.exe").is_file() {
            return ShellKind::Pwsh;
        }
    }
    for dir in std::env::split_paths(&path_var) {
        if dir.join("powershell.exe").is_file() {
            return ShellKind::Powershell;
        }
    }
    ShellKind::Cmd
}

#[cfg(unix)]
pub fn detect_shell() -> ShellKind {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    ShellKind::Custom(PathBuf::from(shell))
}

/// 解析载荷里的 shell 请求："pwsh"/"powershell"/"cmd" 直映射，其余视作可执行路径。
pub fn parse_shell_request(request: &str) -> ShellKind {
    match request.trim().to_ascii_lowercase().as_str() {
        "pwsh" => ShellKind::Pwsh,
        "powershell" => ShellKind::Powershell,
        "cmd" => ShellKind::Cmd,
        other => ShellKind::Custom(PathBuf::from(other)),
    }
}

pub struct ShellSpawn {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// shell 启动命令与参数：编码统一 + venv 徽标提示符经**启动参数**注入
/// （零 stdin 写入 = 零回显、零 400ms 拼接竞态）。
/// PowerShell 系用 `-EncodedCommand`（base64 UTF-16LE，彻底绕开引号/花括号
/// 转义，venv 名特殊字符也安全）；cmd 用 `/k`（命令本身不回显）。
pub fn spawn_command(kind: &ShellKind, venv_name: Option<&str>) -> ShellSpawn {
    match kind {
        ShellKind::Pwsh => ShellSpawn {
            program: PathBuf::from("pwsh.exe"),
            args: ps_args(venv_name),
        },
        ShellKind::Powershell => ShellSpawn {
            program: PathBuf::from("powershell.exe"),
            args: ps_args(venv_name),
        },
        ShellKind::Cmd => ShellSpawn {
            program: PathBuf::from("cmd.exe"),
            args: vec!["/k".into(), "@chcp 65001>nul".into()],
        },
        ShellKind::Custom(path) => ShellSpawn {
            program: path.clone(),
            args: vec![],
        },
    }
}

fn ps_args(venv_name: Option<&str>) -> Vec<String> {
    vec![
        "-NoLogo".into(),
        "-NoExit".into(),
        "-EncodedCommand".into(),
        encode_ps_bootstrap(venv_name),
    ]
}

/// PowerShell 引导脚本（编码统一 + venv 徽标提示符）。
/// InputEncoding 在输入已重定向的场景会抛异常——try/catch 包裹（已知怪癖）。
/// 已知残余：用户 profile 仍先执行，可能覆盖编码设置（应用层不静改 profile）。
fn ps_bootstrap(venv_name: Option<&str>) -> String {
    let mut script = String::from(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8\ntry{[Console]::InputEncoding=[Text.Encoding]::UTF8}catch{}",
    );
    if let Some(name) = venv_name {
        // venv 徽标提示符（覆盖全局 prompt；写 global: 保证跨作用域生效）
        script.push_str(&format!(
            "\nfunction global:prompt {{ \"({name}) $($PWD)> \" }}"
        ));
    }
    script
}

/// UTF-16LE + Base64 编码（-EncodedCommand 载荷格式）。
pub fn encode_ps_bootstrap(venv_name: Option<&str>) -> String {
    let script = ps_bootstrap(venv_name);
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 全量继承当前进程环境，再叠加覆盖项（显式继承不依赖 CommandBuilder 的默认语义）。
pub fn build_command(spawn: &ShellSpawn, cwd: &Path, env_overlay: &HashMap<String, String>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(&spawn.program);
    for arg in &spawn.args {
        cmd.arg(arg);
    }
    cmd.cwd(cwd);
    for (key, value) in std::env::vars_os() {
        cmd.env(key, value);
    }
    for (key, value) in env_overlay {
        cmd.env(key, value);
    }
    cmd
}

/// venv 激活 env（注入式激活核心）：
/// `VIRTUAL_ENV` + PATH 前插 Scripts/bin + `PYTHONUTF8=1`；
/// cmd 侧另用 `PROMPT` 提示符徽标（PowerShell 用内联 prompt 函数，见 default_init_commands）。
pub fn venv_env(venv_root: &Path, venv_name: &str) -> HashMap<String, String> {
    let base_path = std::env::var_os("PATH").unwrap_or_default();
    venv_env_with_path(venv_root, venv_name, &base_path)
}

/// 纯函数版（可单测）：基于给定 PATH 组装激活 env。
pub fn venv_env_with_path(
    venv_root: &Path,
    venv_name: &str,
    base_path: &OsStr,
) -> HashMap<String, String> {
    let scripts = if cfg!(windows) {
        venv_root.join("Scripts")
    } else {
        venv_root.join("bin")
    };
    let sep = if cfg!(windows) { ";" } else { ":" };
    let path = format!(
        "{}{}{}",
        scripts.to_string_lossy(),
        sep,
        base_path.to_string_lossy()
    );
    HashMap::from([
        ("VIRTUAL_ENV".to_string(), venv_root.to_string_lossy().into_owned()),
        ("PATH".to_string(), path),
        ("PYTHONUTF8".to_string(), "1".to_string()),
        // cmd 启动时读取 PROMPT 环境变量；PowerShell 忽略之（其提示符走 prompt 函数）
        ("PROMPT".to_string(), format!("({venv_name}) $P$G")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn venv_env_prepends_scripts_to_path() {
        let base = OsStr::new(r"C:\Windows\System32;C:\Windows");
        let env = venv_env_with_path(Path::new(r"D:\ws\.venv"), ".venv", base);
        assert_eq!(env.get("VIRTUAL_ENV").unwrap(), r"D:\ws\.venv");
        if cfg!(windows) {
            assert_eq!(
                env.get("PATH").unwrap(),
                r"D:\ws\.venv\Scripts;C:\Windows\System32;C:\Windows"
            );
            assert!(env.get("PROMPT").unwrap().starts_with("(.venv) "));
        } else {
            assert_eq!(
                env.get("PATH").unwrap(),
                "D:\\ws\\.venv/bin:C:\\Windows\\System32;C:\\Windows"
            );
        }
        assert_eq!(env.get("PYTHONUTF8").unwrap(), "1");
    }

    #[test]
    fn parse_shell_request_variants() {
        assert_eq!(parse_shell_request("pwsh"), ShellKind::Pwsh);
        assert_eq!(parse_shell_request("PowerShell"), ShellKind::Powershell);
        assert_eq!(parse_shell_request("cmd"), ShellKind::Cmd);
        assert_eq!(
            parse_shell_request(r"C:\bin\git-bash.exe"),
            ShellKind::Custom(PathBuf::from(r"C:\bin\git-bash.exe"))
        );
    }

    #[test]
    fn spawn_command_uses_launch_args() {
        let ps = spawn_command(&ShellKind::Pwsh, None);
        assert_eq!(ps.args[0], "-NoLogo");
        assert_eq!(ps.args[1], "-NoExit");
        assert_eq!(ps.args[2], "-EncodedCommand");
        // cmd 走 /k，无 stdin 注入
        let cmd = spawn_command(&ShellKind::Cmd, Some("dev"));
        assert_eq!(cmd.args, vec!["/k", "@chcp 65001>nul"]);
        // custom 不注入
        assert!(spawn_command(&ShellKind::Custom(PathBuf::from("sh")), None).args.is_empty());
    }

    #[test]
    fn ps_bootstrap_encoding_roundtrip() {
        use base64::Engine as _;
        for venv in [None, Some("dev"), Some("a-b_9")] {
            let encoded = encode_ps_bootstrap(venv);
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&encoded)
                .expect("valid base64");
            // UTF-16LE 解回
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let script = String::from_utf16(&units).expect("utf-16 roundtrip");
            assert!(script.contains("[Console]::OutputEncoding=[Text.Encoding]::UTF8"));
            assert!(script.contains("[Console]::InputEncoding"));
            match venv {
                Some(name) => assert!(
                    script.contains(&format!("({name})")),
                    "venv badge in prompt: {name}"
                ),
                None => assert!(!script.contains("function global:prompt")),
            }
        }
    }
}
