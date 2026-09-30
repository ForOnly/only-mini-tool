mod appearance;
mod commands;
pub mod domain;
mod errors;
mod infrastructure;
mod repository;
mod services;
mod state;

use appearance::AppearanceHost;
use services::settings_service::SettingsService;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let db = infrastructure::database::Database::new(&handle)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            SettingsService::ensure_defaults(&db)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let debug_enabled = SettingsService::is_debug_enabled(&db)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            infrastructure::logging::init(&handle, debug_enabled)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let preference = SettingsService::get_ui_theme(&db)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let os = appearance::query_os_theme();
            app.manage(AppState { db });
            app.manage(AppearanceHost::new(preference, os));
            appearance::sync(&handle, preference);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_setting,
            commands::set_setting,
            commands::get_appearance,
            commands::read_app_log,
            commands::clear_app_log,
            commands::get_logs_dir,
            commands::open_logs_dir,
            commands::open_devtools,
            commands::recognize_image,
            commands::cancel_recognize,
            commands::get_ocr_settings,
            commands::save_ocr_settings,
            commands::stage_image_file,
            commands::stage_image_bytes,
            commands::save_clipboard_image,
            commands::rotate_image_orientation,
            commands::clear_ocr_temp,
            commands::list_scripts,
            commands::get_script,
            commands::create_script,
            commands::update_script,
            commands::delete_script,
            commands::rename_script,
            commands::get_scripts_settings,
            commands::save_scripts_settings,
            commands::run_script,
            commands::cancel_script_run,
            commands::create_scripts_venv,
            commands::create_script_venv,
            commands::list_script_venvs,
            commands::delete_script_venv,
            commands::format_script_code,
            commands::terminal_create,
            commands::terminal_attach,
            commands::terminal_detach,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_dispose,
            commands::terminal_list,
            commands::resolve_script_terminal,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| match event {
            // 窗口销毁：自动回收该窗口名下终端会话（覆盖子窗正常关闭与 webview 异常消亡）
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } => {
                let disposed =
                    services::terminal::TerminalRegistry::global().dispose_owned_by(&label);
                if disposed > 0 {
                    tracing::info!(
                        "[terminal] disposed {disposed} session(s) owned by window {label}"
                    );
                }
            }
            // app 退出：杀全部终端会话进程树（防 conhost/shell 残留）
            tauri::RunEvent::Exit => {
                services::terminal::TerminalRegistry::global().dispose_all();
            }
            _ => {}
        });
}
