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

/// shell 可执行与启动参数（PowerShell 系加 -NoLogo 抑制横幅）。
pub fn spawn_command(kind: &ShellKind) -> ShellSpawn {
    match kind {
        ShellKind::Pwsh => ShellSpawn {
            program: PathBuf::from("pwsh.exe"),
            args: vec!["-NoLogo".into()],
        },
        ShellKind::Powershell => ShellSpawn {
            program: PathBuf::from("powershell.exe"),
            args: vec!["-NoLogo".into()],
        },
        ShellKind::Cmd => ShellSpawn {
            program: PathBuf::from("cmd.exe"),
            args: vec![],
        },
        ShellKind::Custom(path) => ShellSpawn {
            program: path.clone(),
            args: vec![],
        },
    }
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

/// 默认 init commands：编码统一 + （PowerShell）venv 提示符徽标。
/// 均为内联命令，不执行 .ps1 文件（不受 ExecutionPolicy 限制）。
pub fn default_init_commands(kind: &ShellKind, venv_name: Option<&str>) -> Vec<String> {
    match kind {
        ShellKind::Cmd => vec!["@chcp 65001>nul".to_string()],
        ShellKind::Pwsh | ShellKind::Powershell => {
            let mut cmds = vec![
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8".to_string(),
            ];
            if let Some(name) = venv_name {
                // venv 徽标提示符（覆盖全局 prompt；写 global: 保证跨作用域生效）
                cmds.push(format!(
                    "function global:prompt {{ \"({name}) $($PWD)> \" }}"
                ));
            }
            cmds
        }
        ShellKind::Custom(_) => vec![],
    }
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
    fn spawn_command_flags() {
        assert_eq!(spawn_command(&ShellKind::Pwsh).args, vec!["-NoLogo"]);
        assert!(spawn_command(&ShellKind::Cmd).args.is_empty());
    }

    #[test]
    fn init_commands_cover_encoding() {
        assert_eq!(
            default_init_commands(&ShellKind::Cmd, None),
            vec!["@chcp 65001>nul"]
        );
        let ps = default_init_commands(&ShellKind::Powershell, Some("dev"));
        assert_eq!(ps.len(), 2);
        assert!(ps[0].contains("OutputEncoding"));
        assert!(ps[1].contains("\"(dev)"));
    }
}
