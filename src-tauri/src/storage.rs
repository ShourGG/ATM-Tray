use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use crate::crypto;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Session {
    pub session_token: Option<String>,
    pub device_id: Option<String>,
    pub expires_at: Option<i64>,
    pub quota: Option<i32>,
    pub activation_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedCodes {
    pub codes: Vec<String>,
    pub last_used: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSession {
    pub code: String,
    pub session_token: String,
    pub device_id: String,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultiSession {
    pub sessions: Vec<CodeSession>,
}

lazy_static::lazy_static! {
    static ref CURRENT_SESSION: RwLock<Session> = RwLock::new(Session::default());
    // 迁移标记，避免每次读取都检查
    static ref CODES_MIGRATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static ref SESSIONS_MIGRATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
}

pub fn get_data_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("atm-client");
    path
}

pub fn get_factory_auth_file() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".factory");
    path.push("auth.json");
    path
}

pub fn get_factory_auth_backup_file() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".factory");
    path.push("auth.json.atm_backup");
    path
}

// 备份原有的 auth.json
pub fn backup_factory_auth() {
    let auth_file = get_factory_auth_file();
    let backup_file = get_factory_auth_backup_file();
    
    // 如果备份文件存在，说明上次异常退出，先恢复它
    if backup_file.exists() {
        if let Ok(content) = fs::read_to_string(&backup_file) {
            fs::write(&auth_file, content).ok();
            // 不删除备份文件，保持备份状态
        }
        return; // 已恢复，直接返回
    }
    
    // 正常情况：备份当前的 auth.json
    if auth_file.exists() {
        if let Ok(content) = fs::read_to_string(&auth_file) {
            fs::write(&backup_file, content).ok();
        }
    }
}

// 恢复原有的 auth.json
pub fn restore_factory_auth() {
    let auth_file = get_factory_auth_file();
    let backup_file = get_factory_auth_backup_file();
    
    if backup_file.exists() {
        if let Ok(content) = fs::read_to_string(&backup_file) {
            fs::write(&auth_file, content).ok();
            fs::remove_file(&backup_file).ok();
        }
    }
}

pub fn ensure_data_dir() {
    let dir = get_data_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).ok();
    }
}

pub fn set_session(session: Session) {
    if let Ok(mut s) = CURRENT_SESSION.write() {
        *s = session;
    }
}

pub fn get_session() -> Session {
    CURRENT_SESSION.read().map(|s| s.clone()).unwrap_or_default()
}

pub fn clear_session() {
    if let Ok(mut s) = CURRENT_SESSION.write() {
        *s = Session::default();
    }
}

pub fn is_session_valid() -> bool {
    let session = get_session();
    if session.session_token.is_none() {
        return false;
    }
    
    if let Some(expires_at) = session.expires_at {
        let now = chrono::Utc::now().timestamp();
        if now >= expires_at {
            return false;
        }
    }
    
    true
}

pub fn sync_to_factory_auth(access_token: &str, refresh_token: &str) -> Result<String, String> {
    sync_to_factory_auth_with_id(access_token, refresh_token, None)
}

pub fn sync_to_factory_auth_with_id(access_token: &str, refresh_token: &str, token_id: Option<&str>) -> Result<String, String> {
    let file = get_factory_auth_file();
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    
    let auth_data = if let Some(id) = token_id {
        serde_json::json!({
            "access_token": access_token,
            "refresh_token": refresh_token,
            "token_id": id,
            "updated_at": chrono::Utc::now().timestamp()
        })
    } else {
        serde_json::json!({
            "access_token": access_token,
            "refresh_token": refresh_token
        })
    };
    
    let content = serde_json::to_string_pretty(&auth_data).map_err(|e| e.to_string())?;
    fs::write(&file, content).map_err(|e| e.to_string())?;
    
    Ok(file.to_string_lossy().to_string())
}

// 获取当前激活的 token_id
pub fn get_active_token_id() -> Option<String> {
    let file = get_factory_auth_file();
    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                return data.get("token_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }
    }
    None
}

// 获取 auth.json 的更新时间
pub fn get_auth_updated_at() -> Option<i64> {
    let file = get_factory_auth_file();
    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                return data.get("updated_at").and_then(|v| v.as_i64());
            }
        }
    }
    None
}

// 清除 Factory auth.json 文件
pub fn clear_factory_auth() {
    let file = get_factory_auth_file();
    if file.exists() {
        fs::remove_file(&file).ok();
    }
}

// ==================== 激活码本地存储（加密） ====================

fn get_codes_file() -> PathBuf {
    let mut path = get_data_dir();
    path.push("codes.enc"); // 改为 .enc 扩展名表示加密
    path
}

// 迫移旧的明文文件到加密格式（只执行一次）
fn migrate_codes_if_needed() {
    use std::sync::atomic::Ordering;
    if CODES_MIGRATED.load(Ordering::Relaxed) {
        return;
    }
    CODES_MIGRATED.store(true, Ordering::Relaxed);
    
    let mut old_path = get_data_dir();
    old_path.push("codes.json");
    
    if old_path.exists() {
        if let Ok(content) = fs::read_to_string(&old_path) {
            if let Ok(saved) = serde_json::from_str::<SavedCodes>(&content) {
                save_codes_encrypted(&saved);
                fs::remove_file(&old_path).ok();
            }
        }
    }
}

fn save_codes_encrypted(saved: &SavedCodes) {
    let file = get_codes_file();
    if let Ok(json) = serde_json::to_string(saved) {
        if let Ok(encrypted) = crypto::encrypt_local_data(&json) {
            fs::write(&file, encrypted).ok();
        }
    }
}

pub fn save_activation_code(code: &str) {
    let mut saved = load_saved_codes();
    
    // 避免重复添加
    if !saved.codes.contains(&code.to_string()) {
        saved.codes.push(code.to_string());
    }
    saved.last_used = Some(code.to_string());
    
    save_codes_encrypted(&saved);
}

pub fn load_saved_codes() -> SavedCodes {
    // 先检查是否需要迁移旧文件
    migrate_codes_if_needed();
    
    let file = get_codes_file();
    if file.exists() {
        if let Ok(encrypted) = fs::read_to_string(&file) {
            if let Ok(json) = crypto::decrypt_local_data(&encrypted) {
                if let Ok(saved) = serde_json::from_str(&json) {
                    return saved;
                }
            }
        }
    }
    SavedCodes::default()
}

pub fn remove_activation_code(code: &str) {
    let mut saved = load_saved_codes();
    saved.codes.retain(|c| c != code);
    if saved.last_used.as_deref() == Some(code) {
        saved.last_used = saved.codes.first().cloned();
    }
    
    save_codes_encrypted(&saved);
    
    // 同时删除该激活码的会话
    remove_code_session(code);
}

pub fn clear_saved_codes() {
    let file = get_codes_file();
    if file.exists() {
        fs::remove_file(&file).ok();
    }
    // 也删除旧的明文文件
    let mut old_file = get_data_dir();
    old_file.push("codes.json");
    if old_file.exists() {
        fs::remove_file(&old_file).ok();
    }
}

// ==================== 多会话管理（加密） ====================

fn get_sessions_file() -> PathBuf {
    let mut path = get_data_dir();
    path.push("sessions.enc"); // 改为 .enc 扩展名表示加密
    path
}

// 迫移旧的明文文件到加密格式（只执行一次）
fn migrate_sessions_if_needed() {
    use std::sync::atomic::Ordering;
    if SESSIONS_MIGRATED.load(Ordering::Relaxed) {
        return;
    }
    SESSIONS_MIGRATED.store(true, Ordering::Relaxed);
    
    let mut old_path = get_data_dir();
    old_path.push("sessions.json");
    
    if old_path.exists() {
        if let Ok(content) = fs::read_to_string(&old_path) {
            if let Ok(multi) = serde_json::from_str::<MultiSession>(&content) {
                save_sessions_encrypted(&multi);
                fs::remove_file(&old_path).ok();
            }
        }
    }
}

fn save_sessions_encrypted(multi: &MultiSession) {
    let file = get_sessions_file();
    if let Ok(json) = serde_json::to_string(multi) {
        if let Ok(encrypted) = crypto::encrypt_local_data(&json) {
            fs::write(&file, encrypted).ok();
        }
    }
}

pub fn save_code_session(code: &str, session_token: &str, device_id: &str, expires_at: Option<i64>) {
    let mut multi = load_multi_session();
    
    // 移除旧的同激活码会话
    multi.sessions.retain(|s| s.code != code);
    
    // 添加新会话
    multi.sessions.push(CodeSession {
        code: code.to_string(),
        session_token: session_token.to_string(),
        device_id: device_id.to_string(),
        expires_at,
    });
    
    save_sessions_encrypted(&multi);
}

// 更新所有会话的过期时间（心跳续期时调用）
pub fn update_sessions_expiry(new_expires: i64) {
    let mut multi = load_multi_session();
    
    for session in &mut multi.sessions {
        session.expires_at = Some(new_expires);
    }
    
    save_sessions_encrypted(&multi);
    
    #[cfg(debug_assertions)]
    println!("[Storage] 更新所有会话过期时间为: {}", new_expires);
}

pub fn load_multi_session() -> MultiSession {
    // 先检查是否需要迁移旧文件
    migrate_sessions_if_needed();
    
    let file = get_sessions_file();
    if file.exists() {
        match fs::read_to_string(&file) {
            Ok(encrypted) => {
                match crypto::decrypt_local_data(&encrypted) {
                    Ok(json) => {
                        match serde_json::from_str(&json) {
                            Ok(multi) => {
                                #[cfg(debug_assertions)]
                                println!("[Storage] 会话加载成功");
                                return multi;
                            }
                            Err(e) => {
                                #[cfg(debug_assertions)]
                                println!("[Storage] 会话解析失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        println!("[Storage] 会话解密失败: {}", e);
                    }
                }
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                println!("[Storage] 会话文件读取失败: {}", e);
            }
        }
    } else {
        #[cfg(debug_assertions)]
        println!("[Storage] 会话文件不存在");
    }
    MultiSession::default()
}

pub fn remove_code_session(code: &str) {
    let mut multi = load_multi_session();
    multi.sessions.retain(|s| s.code != code);
    
    save_sessions_encrypted(&multi);
}

pub fn clear_all_sessions() {
    let file = get_sessions_file();
    if file.exists() {
        fs::remove_file(&file).ok();
    }
    // 也删除旧的明文文件
    let mut old_file = get_data_dir();
    old_file.push("sessions.json");
    if old_file.exists() {
        fs::remove_file(&old_file).ok();
    }
}

pub fn get_all_valid_sessions() -> Vec<CodeSession> {
    let multi = load_multi_session();
    let now = chrono::Utc::now().timestamp();
    
    #[cfg(debug_assertions)]
    println!("[Storage] 当前时间戳: {}, 会话总数: {}", now, multi.sessions.len());
    
    let valid: Vec<CodeSession> = multi.sessions.into_iter().filter(|s| {
        let is_valid = s.expires_at.map(|e| e > now).unwrap_or(true);
        #[cfg(debug_assertions)]
        println!("[Storage] 会话 {}: expires_at={:?}, valid={}", s.code, s.expires_at, is_valid);
        is_valid
    }).collect();
    
    #[cfg(debug_assertions)]
    println!("[Storage] 有效会话数: {}", valid.len());
    
    valid
}

// ==================== 双激活码模式管理 ====================

// 激活码类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub code: String,
    pub session_token: String,
    pub expires_at: Option<i64>,
    pub is_auto_switch: bool,
}

fn get_license_normal_file() -> std::path::PathBuf {
    let mut path = get_data_dir();
    path.push("license_normal.enc");  // 加密存储
    path
}

fn get_license_autoswitch_file() -> std::path::PathBuf {
    let mut path = get_data_dir();
    path.push("license_autoswitch.enc");  // 加密存储
    path
}

fn get_current_mode_file() -> std::path::PathBuf {
    let mut path = get_data_dir();
    path.push("current_mode.dat");
    path
}

// 保存普通激活码（加密）
pub fn save_normal_license(info: &LicenseInfo) {
    let file = get_license_normal_file();
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string(info) {
        if let Ok(encrypted) = crypto::encrypt_local_data(&json) {
            fs::write(&file, encrypted).ok();
        }
    }
    // 删除旧的明文文件（迁移）
    let mut old_file = get_data_dir();
    old_file.push("license_normal.json");
    fs::remove_file(&old_file).ok();
}

// 保存自动切换激活码（加密）
pub fn save_autoswitch_license(info: &LicenseInfo) {
    let file = get_license_autoswitch_file();
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string(info) {
        if let Ok(encrypted) = crypto::encrypt_local_data(&json) {
            fs::write(&file, encrypted).ok();
        }
    }
    // 删除旧的明文文件（迁移）
    let mut old_file = get_data_dir();
    old_file.push("license_autoswitch.json");
    fs::remove_file(&old_file).ok();
}

// 获取普通激活码（解密）
pub fn get_normal_license() -> Option<LicenseInfo> {
    // 先尝试读取加密文件
    let file = get_license_normal_file();
    if file.exists() {
        if let Ok(encrypted) = fs::read_to_string(&file) {
            if let Ok(json) = crypto::decrypt_local_data(&encrypted) {
                return serde_json::from_str(&json).ok();
            }
        }
    }
    // 兼容旧的明文文件（迁移）
    let mut old_file = get_data_dir();
    old_file.push("license_normal.json");
    if old_file.exists() {
        if let Ok(content) = fs::read_to_string(&old_file) {
            if let Ok(info) = serde_json::from_str::<LicenseInfo>(&content) {
                save_normal_license(&info); // 迁移到加密存储
                return Some(info);
            }
        }
    }
    None
}

// 获取自动切换激活码（解密）
pub fn get_autoswitch_license() -> Option<LicenseInfo> {
    // 先尝试读取加密文件
    let file = get_license_autoswitch_file();
    if file.exists() {
        if let Ok(encrypted) = fs::read_to_string(&file) {
            if let Ok(json) = crypto::decrypt_local_data(&encrypted) {
                return serde_json::from_str(&json).ok();
            }
        }
    }
    // 兼容旧的明文文件（迁移）
    let mut old_file = get_data_dir();
    old_file.push("license_autoswitch.json");
    if old_file.exists() {
        if let Ok(content) = fs::read_to_string(&old_file) {
            if let Ok(info) = serde_json::from_str::<LicenseInfo>(&content) {
                save_autoswitch_license(&info); // 迁移到加密存储
                return Some(info);
            }
        }
    }
    None
}

// 检查是否同时拥有两种激活码
pub fn has_both_licenses() -> bool {
    get_normal_license().is_some() && get_autoswitch_license().is_some()
}

// 保存当前模式 ("normal" 或 "autoswitch")
pub fn save_current_mode(mode: &str) {
    let file = get_current_mode_file();
    fs::write(&file, mode).ok();
}

// 获取当前模式
pub fn get_current_mode() -> String {
    let file = get_current_mode_file();
    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            let mode = content.trim();
            if mode == "autoswitch" || mode == "normal" {
                return mode.to_string();
            }
        }
    }
    "normal".to_string()
}

// 清除指定类型的激活码
pub fn clear_license(is_auto_switch: bool) {
    let file = if is_auto_switch {
        get_license_autoswitch_file()
    } else {
        get_license_normal_file()
    };
    fs::remove_file(&file).ok();
}

// 清除所有激活码（退出登录时）
pub fn clear_all_licenses() {
    // 删除加密文件
    fs::remove_file(get_license_normal_file()).ok();
    fs::remove_file(get_license_autoswitch_file()).ok();
    fs::remove_file(get_current_mode_file()).ok();
    // 删除旧的明文文件
    let mut old_normal = get_data_dir();
    old_normal.push("license_normal.json");
    fs::remove_file(&old_normal).ok();
    let mut old_autoswitch = get_data_dir();
    old_autoswitch.push("license_autoswitch.json");
    fs::remove_file(&old_autoswitch).ok();
}

// ==================== 兼容旧的自动切换设置（已弃用）====================

fn get_auto_switch_file() -> std::path::PathBuf {
    let mut path = get_data_dir();
    path.push("auto_switch.dat");
    path
}

pub fn save_auto_switch(enabled: bool) {
    let file = get_auto_switch_file();
    let value = if enabled { "1" } else { "0" };
    fs::write(&file, value).ok();
}

pub fn get_auto_switch() -> bool {
    let file = get_auto_switch_file();
    if file.exists() {
        if let Ok(content) = fs::read_to_string(&file) {
            return content.trim() == "1";
        }
    }
    false
}
