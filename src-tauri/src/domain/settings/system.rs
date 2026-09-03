//! 系统级 settings 键（ui.* / app.*）。

/// 主题偏好：`system` / `light` / `dark`。
pub const SETTING_UI_THEME: &str = "ui.theme";

/// 可选调试：`"true"` / `"false"`。开启后设置页可查看日志；启动时 DEBUG 级别写文件。
pub const SETTING_DEBUG_ENABLED: &str = "app.debug_enabled";
