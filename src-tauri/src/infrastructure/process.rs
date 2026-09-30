//! 子进程 spawn 统一封装：Windows GUI 父进程 spawn 控制台子进程必须显式
//! 抑制 console 分配——flags=0 时每次 spawn 都走「新分配 console（conhost）」
//! 路径（同时闪窗），高频分配/释放叠加 ConPTY 资源累积后会在 CSRSS/conhost
//! 初始化阶段间歇性失败，子进程以 `STATUS_DLL_INIT_FAILED (0xC0000142)`
//! 终止（实测：终端会话存活期间 format 连续 4 次失败，exit=-1073741502）。
//! stdio 全管道时 `CREATE_NO_WINDOW` 无任何语义损失。

/// Windows CREATE_NO_WINDOW。
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// tokio Command 版（runner / format_code / probe_python；creation_flags 为
/// tokio Windows 固有方法）。
#[cfg(windows)]
pub fn no_window(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn no_window(_cmd: &mut tokio::process::Command) {}

/// std Command 版（kill_tree 的 taskkill）。
#[cfg(windows)]
pub fn no_window_std(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn no_window_std(_cmd: &mut std::process::Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_no_window_flag_value() {
        // Windows SDK 常量防手滑
        assert_eq!(CREATE_NO_WINDOW, 0x0800_0000);
    }
}
