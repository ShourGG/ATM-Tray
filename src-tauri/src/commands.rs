use crate::api;
use crate::crypto;
use crate::security;
use crate::storage::{self, Session};
use serde_json::{json, Value};
use tauri::Emitter;

const CURRENT_VERSION: &str = "2.2.1";

// 语义化版本比较：判断 server_ver 是否比 current_ver 新
fn is_newer_version(server_ver: &str, current_ver: &str) -> bool {
    let parse_version = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };
    
    let sv = parse_version(server_ver);
    let cv = parse_version(current_ver);
    
    for i in 0..sv.len().max(cv.len()) {
        let s = sv.get(i).copied().unwrap_or(0);
        let c = cv.get(i).copied().unwrap_or(0);
        if s > c { return true; }
        if s < c { return false; }
    }
    false
}

// UTF-8 安全的字符串前缀截取（防止多字节字符切片导致 panic）
#[inline]
fn safe_token_prefix(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

#[tauri::command]
pub async fn activate_license(code: String) -> Result<Value, String> {
    let device_id = crypto::get_device_fingerprint();
    
    #[cfg(debug_assertions)]
    println!("[activate_license] 激活码: {}, 设备ID: {}", &code, &device_id);
    
    match api::activate_license(&code, &device_id).await {
        Ok(response) => {
            #[cfg(debug_assertions)]
            println!("[activate_license] 响应: success={}", response.success);
            if response.success {
                // 保存激活码到本地
                storage::save_activation_code(&code);
                
                // 保存多会话
                if let Some(ref token) = response.session_token {
                    #[cfg(debug_assertions)]
                    println!("[activate_license] 保存会话: token前10字符={}", safe_token_prefix(token, 10));
                    storage::save_code_session(&code, token, &device_id, response.expires_at);
                }
                
                // 保存 auto_switch 设置（兼容旧逻辑）
                let auto_switch = response.auto_switch.unwrap_or(false);
                storage::save_auto_switch(auto_switch);
                
                // 保存激活码信息到新的双模式存储
                let license_info = storage::LicenseInfo {
                    code: code.clone(),
                    session_token: response.session_token.clone().unwrap_or_default(),
                    expires_at: response.expires_at,
                    is_auto_switch: auto_switch,
                };
                
                if auto_switch {
                    storage::save_autoswitch_license(&license_info);
                } else {
                    storage::save_normal_license(&license_info);
                }
                
                let session = Session {
                    session_token: response.session_token.clone(),
                    device_id: Some(device_id),
                    expires_at: response.expires_at,
                    quota: response.quota,
                    activation_code: Some(code),
                };
                storage::set_session(session);
                security::set_session_valid(true);
                
                // 检查是否同时拥有两种激活码
                let has_both = storage::has_both_licenses();
                
                Ok(json!({
                    "success": true,
                    "quota": response.quota,
                    "expiresAt": response.expires_at,
                    "autoSwitch": auto_switch,
                    "hasBothLicenses": has_both
                }))
            } else {
                Ok(json!({
                    "success": false,
                    "error": response.error.unwrap_or_else(|| "激活失败".to_string())
                }))
            }
        }
        Err(e) => Ok(json!({
            "success": false,
            "error": e
        }))
    }
}

#[tauri::command]
pub fn get_session_status() -> Result<Value, String> {
    let session = storage::get_session();
    let is_valid = storage::is_session_valid();
    
    Ok(json!({
        "isLoggedIn": is_valid,
        "quota": session.quota,
        "expiresAt": session.expires_at
    }))
}

#[tauri::command]
pub fn logout() -> Result<Value, String> {
    storage::clear_session();
    security::set_session_valid(false);
    
    Ok(json!({
        "success": true
    }))
}

// 解除设备绑定并清理数据
#[tauri::command]
pub async fn unbind_and_clear() -> Result<Value, String> {
    let device_id = crypto::get_device_fingerprint();
    let sessions = storage::get_all_valid_sessions();
    
    let mut unbind_results: Vec<String> = Vec::new();
    
    // 尝试解绑所有保存的激活码
    for session in &sessions {
        match api::unbind_device(&session.code, &device_id).await {
            Ok(_) => {
                unbind_results.push(format!("{}: 解绑成功", &session.code));
            }
            Err(e) => {
                unbind_results.push(format!("{}: {}", &session.code, e));
            }
        }
    }
    
    // 清除本地数据
    storage::clear_session();
    storage::clear_all_sessions();
    storage::clear_saved_codes();
    storage::clear_factory_auth();
    security::set_session_valid(false);
    
    Ok(json!({
        "success": true,
        "results": unbind_results
    }))
}

#[tauri::command]
pub fn clear_all_data() -> Result<Value, String> {
    // 清除所有会话
    storage::clear_session();
    storage::clear_all_sessions();
    storage::clear_saved_codes();
    security::set_session_valid(false);
    
    Ok(json!({
        "success": true
    }))
}

#[tauri::command]
pub async fn get_token_list() -> Result<Value, String> {
    let session = storage::get_session();
    
    let session_token = session.session_token.ok_or("未登录")?;
    let device_id = session.device_id.ok_or("设备ID缺失")?;
    
    match api::get_token_list(&session_token, &device_id).await {
        Ok(tokens) => Ok(json!({
            "success": true,
            "data": tokens
        })),
        Err(e) => {
            if e == "SESSION_EXPIRED" {
                storage::clear_session();
                security::set_session_valid(false);
            }
            Ok(json!({
                "success": false,
                "error": e
            }))
        }
    }
}

#[tauri::command]
pub async fn activate_token(token_id: String) -> Result<Value, String> {
    // 从多会话中尝试激活
    let sessions = storage::get_all_valid_sessions();
    
    #[cfg(debug_assertions)]
    {
        println!("[activate_token] 开始激活, token_id: {}", token_id);
        println!("[activate_token] 有效会话数: {}", sessions.len());
    }
    
    if sessions.is_empty() {
        #[cfg(debug_assertions)]
        println!("[activate_token] 错误: 没有有效会话");
        return Ok(json!({
            "success": false,
            "error": "未登录"
        }));
    }
    
    // 遍历所有会话，尝试激活
    let mut last_error = String::new();
    for session in sessions {
        #[cfg(debug_assertions)]
        println!("[activate_token] 尝试会话: {}, token前10字符: {}", &session.code, safe_token_prefix(&session.session_token, 10));
        match api::activate_token(&session.session_token, &token_id, &session.device_id).await {
            Ok((access_token, refresh_token)) => {
                #[cfg(debug_assertions)]
                println!("[activate_token] 激活成功!");
                // 写入本地 auth.json（包含 token_id 以便后续自动刷新）
                let path = storage::sync_to_factory_auth_with_id(&access_token, &refresh_token, Some(&token_id))?;
                
                return Ok(json!({
                    "success": true,
                    "path": path
                }));
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                println!("[activate_token] 会话失败: {}", &e);
                last_error = e.clone();
                if e == "SESSION_EXPIRED" {
                    storage::remove_code_session(&session.code);
                }
                // 继续尝试下一个会话
                continue;
            }
        }
    }
    
    // 所有会话都失败
    #[cfg(debug_assertions)]
    println!("[activate_token] 所有会话都失败, 最后错误: {}", &last_error);
    Ok(json!({
        "success": false,
        "error": format!("激活失败: {}", last_error)
    }))
}

#[tauri::command]
pub async fn get_subscription(token_id: String) -> Result<Value, String> {
    // 从多会话中尝试
    let sessions = storage::get_all_valid_sessions();
    
    if sessions.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "未登录"
        }));
    }
    
    // 遍历所有会话，尝试获取
    for session in sessions {
        match api::activate_token(&session.session_token, &token_id, &session.device_id).await {
            Ok((access_token, _)) => {
                // 查询余额
                match api::get_subscription(&access_token).await {
                    Ok(data) => return Ok(json!({
                        "success": true,
                        "data": data
                    })),
                    Err(e) => return Ok(json!({
                        "success": false,
                        "error": e
                    }))
                }
            }
            Err(e) => {
                if e == "SESSION_EXPIRED" {
                    storage::remove_code_session(&session.code);
                }
                continue;
            }
        }
    }
    
    Ok(json!({
        "success": false,
        "error": "无法获取，请重新添加激活码"
    }))
}

#[tauri::command]
pub async fn heartbeat() -> Result<Value, String> {
    // 优先使用文件存储的会话（应用重启后内存会话为空）
    let sessions = storage::get_all_valid_sessions();
    
    if sessions.is_empty() {
        // 尝试内存中的会话（兼容旧逻辑）
        let session = storage::get_session();
        if let (Some(session_token), Some(device_id)) = (session.session_token, session.device_id) {
            match api::heartbeat(&session_token, &device_id).await {
                Ok(resp) => {
                    if !resp.valid {
                        storage::clear_session();
                        security::set_session_valid(false);
                    } else if let Some(new_expires) = resp.expires_at {
                        storage::update_sessions_expiry(new_expires);
                    }
                    return Ok(json!({ "valid": resp.valid }));
                }
                Err(_) => return Ok(json!({ "valid": false }))
            }
        }
        return Ok(json!({ "valid": false }));
    }
    
    // 使用第一个有效会话发送心跳
    let first_session = &sessions[0];
    match api::heartbeat(&first_session.session_token, &first_session.device_id).await {
        Ok(resp) => {
            if !resp.valid {
                // 会话无效，清理
                storage::remove_code_session(&first_session.code);
                security::set_session_valid(false);
            } else if let Some(new_expires) = resp.expires_at {
                // 更新所有会话的过期时间
                storage::update_sessions_expiry(new_expires);
            }
            Ok(json!({ "valid": resp.valid }))
        }
        Err(_) => Ok(json!({ "valid": false }))
    }
}

#[tauri::command]
pub fn get_device_id() -> Result<String, String> {
    Ok(crypto::get_device_fingerprint())
}

#[tauri::command]
pub async fn hide_window(window: tauri::Window) -> Result<(), String> {
    let _ = window.set_skip_taskbar(true); // 隐藏时移除任务栏图标
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn exit_app(app: tauri::AppHandle) -> Result<(), String> {
    storage::restore_factory_auth();
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn get_app_info() -> Result<Value, String> {
    Ok(json!({
        "version": CURRENT_VERSION
    }))
}

#[tauri::command]
pub fn get_saved_codes() -> Result<Value, String> {
    let saved = storage::load_saved_codes();
    Ok(json!({
        "codes": saved.codes,
        "lastUsed": saved.last_used
    }))
}

#[tauri::command]
pub fn remove_saved_code(code: String) -> Result<Value, String> {
    storage::remove_activation_code(&code);
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub async fn get_all_tokens() -> Result<Value, String> {
    let all_sessions = storage::get_all_valid_sessions();
    
    if all_sessions.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "无有效会话"
        }));
    }
    
    // 获取当前模式和对应的激活码
    let current_mode = storage::get_current_mode();
    let target_license = if current_mode == "autoswitch" {
        storage::get_autoswitch_license()
    } else {
        storage::get_normal_license()
    };
    
    // 过滤出当前模式对应的会话
    let sessions: Vec<_> = if let Some(license) = target_license {
        all_sessions.into_iter()
            .filter(|s| s.code == license.code)
            .collect()
    } else {
        // 如果没有对应模式的激活码，返回所有会话（兼容旧逻辑）
        all_sessions
    };
    
    if sessions.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "当前模式无有效会话"
        }));
    }
    
    // 并行请求所有会话的 token 列表（大幅提升加载速度）
    let futures: Vec<_> = sessions.iter().map(|session| {
        let session_token = session.session_token.clone();
        let device_id = session.device_id.clone();
        let code = session.code.clone();
        async move {
            let result = api::get_token_list(&session_token, &device_id).await;
            (code, result)
        }
    }).collect();
    
    let results = futures::future::join_all(futures).await;
    
    let mut all_tokens: Vec<api::TokenInfo> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    
    for (code, result) in results {
        match result {
            Ok(tokens) => {
                for token in tokens {
                    if !all_tokens.iter().any(|t| t.id == token.id) {
                        all_tokens.push(token);
                    }
                }
            }
            Err(e) => {
                if e == "SESSION_EXPIRED" {
                    storage::remove_code_session(&code);
                }
                errors.push(format!("{}: {}", code, e));
            }
        }
    }
    
    if all_tokens.is_empty() && !errors.is_empty() {
        return Ok(json!({
            "success": false,
            "error": errors.join(", ")
        }));
    }
    
    Ok(json!({
        "success": true,
        "data": all_tokens
    }))
}

// 自动刷新当前激活的 token（检查服务器版本，版本变化立即同步）
#[tauri::command]
pub async fn refresh_active_token(force: bool) -> Result<Value, String> {
    // 检查是否有激活的 token_id
    let token_id = match storage::get_active_token_id() {
        Some(id) => id,
        None => return Ok(json!({ "success": false, "error": "无激活的token" }))
    };
    
    let local_updated_at = storage::get_auth_updated_at().unwrap_or(0);
    
    // 获取有效会话
    let sessions = storage::get_all_valid_sessions();
    if sessions.is_empty() {
        return Ok(json!({ "success": false, "error": "无有效会话" }));
    }
    
    // 检查服务器上的 token 版本
    let mut server_updated_at: i64 = 0;
    let mut valid_session: Option<&storage::CodeSession> = None;
    
    for session in &sessions {
        match api::check_token_version(&session.session_token, &token_id, &session.device_id).await {
            Ok(updated_at) => {
                server_updated_at = updated_at;
                valid_session = Some(session);
                break;
            }
            Err(e) => {
                if e == "SESSION_EXPIRED" {
                    storage::remove_code_session(&session.code);
                }
                continue;
            }
        }
    }
    
    // 如果没有找到有效会话
    let session = match valid_session {
        Some(s) => s,
        None => return Ok(json!({ "success": false, "error": "无有效会话" }))
    };
    
    #[cfg(debug_assertions)]
    println!("[refresh_active_token] 本地版本: {}, 服务器版本: {}", local_updated_at, server_updated_at);
    
    // 如果服务器版本更新或强制刷新，则重新获取
    if !force && server_updated_at <= local_updated_at && local_updated_at > 0 {
        #[cfg(debug_assertions)]
        println!("[refresh_active_token] 本地已是最新版本，跳过刷新");
        return Ok(json!({ "success": true, "skipped": true }));
    }
    
    #[cfg(debug_assertions)]
    println!("[refresh_active_token] 服务器有更新，开始同步 token_id: {}", &token_id);
    
    // 从服务器获取最新 token
    match api::activate_token(&session.session_token, &token_id, &session.device_id).await {
        Ok((access_token, refresh_token)) => {
            storage::sync_to_factory_auth_with_id(&access_token, &refresh_token, Some(&token_id))?;
            #[cfg(debug_assertions)]
            println!("[refresh_active_token] 同步成功!");
            return Ok(json!({ "success": true, "refreshed": true }));
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            println!("[refresh_active_token] 同步失败: {}", &e);
            return Ok(json!({ "success": false, "error": e }));
        }
    }
}

// 检查客户端更新
#[tauri::command]
pub async fn check_update() -> Result<Value, String> {
    match api::check_update().await {
        Ok(info) => {
            if !info.has_update || info.version.is_none() {
                return Ok(json!({ "hasUpdate": false }));
            }
            
            let server_version = info.version.as_ref().unwrap();
            let sv = server_version.trim_start_matches('v');
            let cv = CURRENT_VERSION.trim_start_matches('v');
            
            if !is_newer_version(sv, cv) {
                return Ok(json!({ "hasUpdate": false }));
            }
            
            Ok(json!({
                "hasUpdate": true,
                "version": info.version,
                "changelog": info.changelog,
                "size": info.size,
                "forceUpdate": info.force_update.unwrap_or(false),
                "downloadUrl": info.download_url
            }))
        }
        Err(e) => {
            Ok(json!({ "hasUpdate": false, "error": e }))
        }
    }
}

// 打开浏览器下载更新
#[tauri::command]
pub async fn open_download_url(download_url: String) -> Result<Value, String> {
    // 构建完整 URL
    let full_url = if download_url.starts_with("http") {
        download_url
    } else if download_url.starts_with("/client/") {
        format!("{}{}", api::get_api_base(), download_url)
    } else {
        format!("{}{}", api::get_api_base(), download_url)
    };
    
    // 打开浏览器
    opener::open(&full_url)
        .map_err(|e| format!("打开浏览器失败: {}", e))?;
    
    Ok(json!({ "success": true }))
}

// 下载并执行更新（方案三：重命名策略）
#[tauri::command]
pub async fn download_and_update(
    app: tauri::AppHandle,
    download_url: String,
) -> Result<Value, String> {
    // macOS 暂不支持自动更新，请手动下载
    #[cfg(target_os = "macos")]
    {
        return Err("macOS 版本请手动下载更新".to_string());
    }
    
    #[cfg(not(target_os = "macos"))]
    {
    use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
    
    // 获取当前 exe 路径
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let exe_dir = current_exe.parent()
        .ok_or("无法获取程序目录")?;
    let exe_name = current_exe.file_name()
        .ok_or("无法获取程序名称")?;
    
    // 定义路径
    let new_exe_path = exe_dir.join("update_new.exe");
    let old_exe_path = exe_dir.join(format!("{}.old", exe_name.to_string_lossy()));
    
    // 进度跟踪
    let downloaded = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let downloaded_clone = Arc::clone(&downloaded);
    let total_clone = Arc::clone(&total);
    let app_clone = app.clone();
    
    // 进度回调
    let progress_callback = move |dl: u64, tl: u64| {
        downloaded_clone.store(dl, Ordering::SeqCst);
        total_clone.store(tl, Ordering::SeqCst);
        // 发送进度事件到前端
        let _ = app_clone.emit("update-progress", json!({
            "downloaded": dl,
            "total": tl,
            "percent": if tl > 0 { (dl as f64 / tl as f64 * 100.0) as u32 } else { 0 }
        }));
    };
    
    // 下载新版本
    api::download_update(&download_url, &new_exe_path, progress_callback).await?;
    
    // 验证下载的文件
    let metadata = std::fs::metadata(&new_exe_path)
        .map_err(|e| format!("验证下载文件失败: {}", e))?;
    if metadata.len() < 1024 * 100 {
        // 文件小于 100KB，可能下载失败
        let _ = std::fs::remove_file(&new_exe_path);
        return Err("下载的文件异常，请重试".to_string());
    }
    
    // 重命名策略：
    // 1. 当前运行的 exe 重命名为 .old（Windows 允许重命名运行中的文件）
    // 2. 新下载的 exe 重命名为原名称
    // 3. 启动新版本
    // 4. 退出当前程序
    
    // 步骤1：重命名当前 exe 为 .old
    if let Err(e) = std::fs::rename(&current_exe, &old_exe_path) {
        let _ = std::fs::remove_file(&new_exe_path);
        return Err(format!("重命名当前程序失败: {}，请关闭其他可能占用的程序", e));
    }
    
    // 步骤2：重命名新 exe 为当前名称
    if let Err(e) = std::fs::rename(&new_exe_path, &current_exe) {
        // 回滚：恢复原来的 exe
        let _ = std::fs::rename(&old_exe_path, &current_exe);
        return Err(format!("安装新版本失败: {}", e));
    }
    
    // 步骤3：启动新版本
    let _ = std::process::Command::new(&current_exe)
        .spawn()
        .map_err(|e| format!("启动新版本失败: {}", e))?;
    
    // 步骤4：退出当前程序
    storage::restore_factory_auth();
    app.exit(0);
    
    Ok(json!({ "success": true }))
    } // #[cfg(not(target_os = "macos"))]
}

// 清理旧版本文件（在启动时调用）
pub fn cleanup_old_version() {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            // 清理 .old 文件
            if let Ok(entries) = std::fs::read_dir(exe_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "old" {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
            // 清理 update_new.exe（下载中断的残留）
            let new_exe = exe_dir.join("update_new.exe");
            if new_exe.exists() {
                let _ = std::fs::remove_file(&new_exe);
            }
        }
    }
}

// ==================== 开机自启动 ====================
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const APP_NAME: &str = "ATM Tray";
const MACOS_BUNDLE_ID: &str = "com.atm-tray.app";

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<Value, String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {}", e))?;
        let exe_path_str = exe_path.to_string_lossy();
        
        if enabled {
            // 添加到注册表
            let output = Command::new("reg")
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v", APP_NAME,
                    "/t", "REG_SZ",
                    "/d", &format!("\"{}\"", exe_path_str),
                    "/f"
                ])
                .output()
                .map_err(|e| format!("执行命令失败: {}", e))?;
            
            if output.status.success() {
                Ok(json!({ "success": true, "enabled": true }))
            } else {
                Err("设置自启动失败".to_string())
            }
        } else {
            // 从注册表删除
            let _output = Command::new("reg")
                .creation_flags(0x08000000)
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v", APP_NAME,
                    "/f"
                ])
                .output()
                .map_err(|e| format!("执行命令失败: {}", e))?;
            
            Ok(json!({ "success": true, "enabled": false }))
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        use std::fs;
        use std::path::PathBuf;
        
        let home = std::env::var("HOME").map_err(|_| "获取HOME目录失败")?;
        let plist_dir = PathBuf::from(&home).join("Library/LaunchAgents");
        let plist_path = plist_dir.join(format!("{}.plist", MACOS_BUNDLE_ID));
        
        if enabled {
            // 创建 LaunchAgents 目录
            fs::create_dir_all(&plist_dir).map_err(|e| format!("创建目录失败: {}", e))?;
            
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("获取程序路径失败: {}", e))?;
            
            // 生成 plist 内容
            let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#, MACOS_BUNDLE_ID, exe_path.to_string_lossy());
            
            fs::write(&plist_path, plist_content)
                .map_err(|e| format!("写入plist失败: {}", e))?;
            
            Ok(json!({ "success": true, "enabled": true }))
        } else {
            // 删除 plist 文件
            if plist_path.exists() {
                fs::remove_file(&plist_path).map_err(|e| format!("删除plist失败: {}", e))?;
            }
            Ok(json!({ "success": true, "enabled": false }))
        }
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(json!({ "success": false, "error": "不支持此操作系统" }))
    }
}

#[tauri::command]
pub fn get_autostart_status() -> Result<Value, String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let output = Command::new("reg")
            .creation_flags(0x08000000)
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v", APP_NAME
            ])
            .output()
            .map_err(|e| format!("查询失败: {}", e))?;
        
        let enabled = output.status.success();
        Ok(json!({ "enabled": enabled }))
    }
    
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "获取HOME目录失败")?;
        let plist_path = std::path::PathBuf::from(&home)
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", MACOS_BUNDLE_ID));
        
        Ok(json!({ "enabled": plist_path.exists() }))
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(json!({ "enabled": false }))
    }
}

// 更新自启动路径（启动时调用，确保路径是最新的）
#[tauri::command]
pub fn update_autostart_path() -> Result<Value, String> {
    // 先检查是否已启用自启动
    let status = get_autostart_status()?;
    if let Some(enabled) = status.get("enabled").and_then(|v| v.as_bool()) {
        if enabled {
            // 重新设置自启动（更新路径）
            return set_autostart(true);
        }
    }
    Ok(json!({ "success": true, "updated": false }))
}

// 获取自动切换状态（兼容旧版本）
#[tauri::command]
pub fn get_auto_switch_status() -> Result<Value, String> {
    let enabled = storage::get_auto_switch();
    Ok(json!({ "enabled": enabled }))
}

// ==================== 双激活码模式管理 ====================

// 检查激活码状态（启动时调用）
#[tauri::command]
pub fn check_license_status() -> Result<Value, String> {
    let normal = storage::get_normal_license();
    let autoswitch = storage::get_autoswitch_license();
    let current_mode = storage::get_current_mode();
    
    let has_normal = normal.is_some();
    let has_autoswitch = autoswitch.is_some();
    let has_both = has_normal && has_autoswitch;
    
    Ok(json!({
        "hasNormal": has_normal,
        "hasAutoswitch": has_autoswitch,
        "hasBoth": has_both,
        "currentMode": current_mode,
        "normalCode": normal.map(|l| l.code),
        "autoswitchCode": autoswitch.map(|l| l.code)
    }))
}

// 设置当前模式
#[tauri::command]
pub fn set_current_mode(mode: String) -> Result<Value, String> {
    if mode != "normal" && mode != "autoswitch" {
        return Ok(json!({
            "success": false,
            "error": "无效的模式"
        }));
    }
    storage::save_current_mode(&mode);
    Ok(json!({
        "success": true,
        "mode": mode
    }))
}

// 获取当前模式
#[tauri::command]
pub fn get_current_mode() -> Result<Value, String> {
    let mode = storage::get_current_mode();
    let is_autoswitch = mode == "autoswitch";
    Ok(json!({
        "mode": mode,
        "isAutoswitch": is_autoswitch
    }))
}

// 清除所有激活码（完全退出登录）
#[tauri::command]
pub fn clear_all_licenses() -> Result<Value, String> {
    storage::clear_all_licenses();
    storage::clear_session();
    storage::clear_all_sessions();
    storage::clear_saved_codes();
    security::set_session_valid(false);
    Ok(json!({ "success": true }))
}

// 获取指定模式的激活码
#[tauri::command]
pub fn get_license_code(mode: String) -> Result<Value, String> {
    let license = if mode == "autoswitch" {
        storage::get_autoswitch_license()
    } else {
        storage::get_normal_license()
    };
    
    match license {
        Some(info) => Ok(json!({
            "success": true,
            "code": info.code
        })),
        None => Ok(json!({
            "success": false,
            "error": "未找到对应激活码"
        }))
    }
}
