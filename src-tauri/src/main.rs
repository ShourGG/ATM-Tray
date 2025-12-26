#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod crypto;
mod security;
mod api;
mod storage;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

fn main() {
    // 反调试检测
    #[cfg(not(debug_assertions))]
    {
        if security::is_debugger_present() {
            std::process::exit(1);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 当尝试打开第二个实例时，显示已有窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_skip_taskbar(false);
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::activate_license,
            commands::get_session_status,
            commands::logout,
            commands::clear_all_data,
            commands::get_token_list,
            commands::activate_token,
            commands::get_subscription,
            commands::heartbeat,
            commands::get_device_id,
            commands::get_app_info,
            commands::get_saved_codes,
            commands::remove_saved_code,
            commands::get_all_tokens,
            commands::hide_window,
            commands::exit_app,
            commands::refresh_active_token,
            commands::unbind_and_clear,
            commands::check_update,
            commands::open_download_url,
            commands::download_and_update,
            commands::set_autostart,
            commands::get_autostart_status,
            commands::update_autostart_path,
            commands::get_auto_switch_status,
            commands::check_license_status,
            commands::set_current_mode,
            commands::get_current_mode,
            commands::clear_all_licenses,
            commands::get_license_code,
        ])
        .setup(|app| {
            // 清理旧版本文件（更新后的残留）
            commands::cleanup_old_version();
            
            storage::ensure_data_dir();
            
            // 备份原有的 auth.json
            storage::backup_factory_auth();
            
            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            
            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.set_skip_taskbar(false); // 显示时恢复任务栏图标
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            storage::restore_factory_auth();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_skip_taskbar(false); // 显示时恢复任务栏图标
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            
            // 启动心跳线程
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                security::start_heartbeat_loop(app_handle);
            });
            
            Ok(())
        })
        .on_window_event(|window, event| {
            // 点击关闭按钮时隐藏窗口而不是退出
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.set_skip_taskbar(true); // 隐藏时移除任务栏图标
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
