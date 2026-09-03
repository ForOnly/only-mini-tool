//! 进程级外观：偏好在设置，解析在宿主，WebView 只认 set_theme。

use std::sync::Mutex;

use tauri::{AppHandle, Manager, Theme};

use crate::domain::{AppearanceDto, ColorScheme, UiTheme};
use crate::services::settings_service::SettingsService;
use crate::state::AppState;

pub struct AppearanceHost {
    inner: Mutex<Snapshot>,
}

#[derive(Clone, Copy)]
struct Snapshot {
    preference: UiTheme,
    os: ColorScheme,
    resolved: ColorScheme,
}

impl AppearanceHost {
    pub fn new(preference: UiTheme, os: ColorScheme) -> Self {
        Self {
            inner: Mutex::new(Snapshot {
                preference,
                os,
                resolved: preference.resolve(os),
            }),
        }
    }

    pub fn snapshot(&self) -> AppearanceDto {
        let snap = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        AppearanceDto {
            preference: snap.preference,
            resolved: snap.resolved,
        }
    }
}

pub fn query_os_theme() -> ColorScheme {
    #[cfg(windows)]
    {
        if let Some(scheme) = os_theme_from_registry() {
            return scheme;
        }
    }
    ColorScheme::Light
}

pub fn sync(app: &AppHandle, preference: UiTheme) {
    let os = query_os_theme();
    let resolved = preference.resolve(os);
    if let Some(host) = app.try_state::<AppearanceHost>() {
        let unchanged = {
            let mut snap = host.inner.lock().unwrap_or_else(|e| e.into_inner());
            let same = snap.preference == preference && snap.os == os && snap.resolved == resolved;
            snap.preference = preference;
            snap.os = os;
            snap.resolved = resolved;
            same
        };
        if unchanged {
            return;
        }
    }
    apply_theme_to_main(app, resolved);
}

pub fn sync_from_settings(app: &AppHandle) {
    let preference = app
        .try_state::<AppState>()
        .and_then(|state| SettingsService::get_ui_theme(&state.db).ok())
        .unwrap_or(UiTheme::System);
    sync(app, preference);
}

fn apply_theme_to_main(app: &AppHandle, scheme: ColorScheme) {
    let want = match scheme {
        ColorScheme::Light => Theme::Light,
        ColorScheme::Dark => Theme::Dark,
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(Some(want));
    }
}

#[cfg(windows)]
fn os_theme_from_registry() -> Option<ColorScheme> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .ok()?;
    let data: u32 = key.get_value("AppsUseLightTheme").ok()?;
    Some(if data == 0 {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    })
}
