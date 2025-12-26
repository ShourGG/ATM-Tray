use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::crypto;
use std::time::Duration;
use futures_util::StreamExt;
use std::io::Write;

// 服务器地址（运行时解密）
#[inline(never)]
pub fn get_api_base() -> String {
    crypto::get_api_url()
}

// 全局复用的 HTTP Client
lazy_static::lazy_static! {
    static ref HTTP_CLIENT: Client = Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(8))
        .pool_max_idle_per_host(5)
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| Client::new());
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivateRequest {
    pub code: String,
    pub device_id: String,
    pub timestamp: i64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivateResponse {
    pub success: bool,
    pub session_token: Option<String>,
    pub expires_at: Option<i64>,
    pub quota: Option<i32>,
    pub error: Option<String>,
    pub auto_switch: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenListResponse {
    pub success: bool,
    pub data: Option<String>,      // 加密的数据
    pub iv: Option<String>,
    pub tag: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub is_valid: bool,
    pub quota_used: Option<i64>,
    pub quota_total: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivateTokenResponse {
    pub success: bool,
    pub email: Option<String>,
    pub access_token: Option<String>,  // 加密的
    pub access_iv: Option<String>,
    pub access_tag: Option<String>,
    pub refresh_token: Option<String>, // 加密的
    pub refresh_iv: Option<String>,
    pub refresh_tag: Option<String>,
    pub error: Option<String>,
}

pub async fn activate_license(code: &str, device_id: &str) -> Result<ActivateResponse, String> {
    let client = &*HTTP_CLIENT;
    let url = format!("{}/auth/activate", get_api_base());
    
    // 重试3次
    for attempt in 1..=3 {
        let timestamp = chrono::Utc::now().timestamp();
        let signature = crypto::generate_signature(code, timestamp, device_id);
        
        let request = ActivateRequest {
            code: code.to_string(),
            device_id: device_id.to_string(),
            timestamp,
            signature,
        };
        
        #[cfg(debug_assertions)]
        println!("[API] 请求 URL: {} (尝试 {}/3)", &url, attempt);
        
        match client.post(&url).json(&request).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    let _text = response.text().await.unwrap_or_default();
                    return Err(format!("服务器错误 ({})", status.as_u16()));
                }
                return response.json().await.map_err(|_| "数据解析失败".to_string());
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                println!("[API] 请求失败 (尝试 {}/3): {:?}", attempt, e);
                
                if attempt == 3 {
                    return Err("网络连接失败，请检查网络".to_string());
                }
                // 等待1秒后重试
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    Err("网络连接失败".to_string())
}

pub async fn get_token_list(session_token: &str, device_id: &str) -> Result<Vec<TokenInfo>, String> {
    let client = &*HTTP_CLIENT;
    let url = format!("{}/tokens", get_api_base());
    
    // 重试3次
    for attempt in 1..=3 {
        let timestamp = chrono::Utc::now().timestamp();
        let signature = crypto::generate_signature(session_token, timestamp, device_id);
        
        let result = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", session_token))
            .header("X-Device-ID", device_id)
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Signature", &signature)
            .send()
            .await;
        
        match result {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    if status.as_u16() == 401 {
                        return Err("SESSION_EXPIRED".to_string());
                    }
                    let _text = response.text().await.unwrap_or_default();
                    return Err(format!("服务器错误 ({})", status.as_u16()));
                }
                
                let resp: TokenListResponse = response.json().await.map_err(|_| "数据解析失败".to_string())?;
                
                if !resp.success {
                    return Err(resp.error.unwrap_or_else(|| "未知错误".to_string()));
                }
                
                // 解密数据
                let encrypted_data = resp.data.ok_or("无数据")?;
                let iv = resp.iv.ok_or("缺少IV")?;
                let tag = resp.tag.ok_or("缺少Tag")?;
                
                let decrypted = crypto::decrypt_payload(&encrypted_data, &iv, &tag)?;
                let tokens: Vec<TokenInfo> = serde_json::from_str(&decrypted)
                    .map_err(|_| "数据解析失败".to_string())?;
                
                return Ok(tokens);
            }
            Err(_) => {
                if attempt == 3 {
                    return Err("网络连接失败".to_string());
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    Err("网络连接失败".to_string())
}

pub async fn activate_token(
    session_token: &str,
    token_id: &str,
    device_id: &str,
) -> Result<(String, String), String> {
    let client = &*HTTP_CLIENT;
    let timestamp = chrono::Utc::now().timestamp();
    // 签名格式与 verifySignature 中间件一致
    let signature = crypto::generate_signature(session_token, timestamp, device_id);
    
    let response = client
        .post(format!("{}/tokens/activate", get_api_base()))
        .header("Authorization", format!("Bearer {}", session_token))
        .header("X-Device-ID", device_id)
        .header("X-Timestamp", timestamp.to_string())
        .header("X-Signature", &signature)
        .json(&serde_json::json!({ "token_id": token_id }))
        .send()
        .await
        .map_err(|_| "网络连接失败".to_string())?;
    
    if !response.status().is_success() {
        let status = response.status();
        if status.as_u16() == 401 {
            return Err("SESSION_EXPIRED".to_string());
        }
        let _text = response.text().await.unwrap_or_default();
        return Err(format!("服务器错误 ({})", status.as_u16()));
    }
    
    let resp: ActivateTokenResponse = response.json().await.map_err(|_| "数据解析失败".to_string())?;
    
    if !resp.success {
        return Err(resp.error.unwrap_or_else(|| "未知错误".to_string()));
    }
    
    // 解密 access_token
    let access_encrypted = resp.access_token.ok_or("缺少access_token")?;
    let access_iv = resp.access_iv.ok_or("缺少access_iv")?;
    let access_tag = resp.access_tag.ok_or("缺少access_tag")?;
    let access_token = crypto::decrypt_payload(&access_encrypted, &access_iv, &access_tag)?;
    
    // 解密 refresh_token
    let refresh_encrypted = resp.refresh_token.ok_or("缺少refresh_token")?;
    let refresh_iv = resp.refresh_iv.ok_or("缺少refresh_iv")?;
    let refresh_tag = resp.refresh_tag.ok_or("缺少refresh_tag")?;
    let refresh_token = crypto::decrypt_payload(&refresh_encrypted, &refresh_iv, &refresh_tag)?;
    
    Ok((access_token, refresh_token))
}

pub async fn get_subscription(access_token: &str) -> Result<Value, String> {
    let client = &*HTTP_CLIENT;
    
    let response = client
        .post(crypto::get_factory_api_url())
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("Accept", "*/*")
        .header("x-factory-client", "web-app")
        .body("{}")
        .send()
        .await
        .map_err(|_| "网络连接失败".to_string())?;
    
    if !response.status().is_success() {
        return Err("查询失败".to_string());
    }
    
    response.json().await.map_err(|_| "数据解析失败".to_string())
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatResponse {
    pub valid: bool,
    pub expires_at: Option<i64>,
}

pub async fn heartbeat(session_token: &str, device_id: &str) -> Result<HeartbeatResponse, String> {
    let client = &*HTTP_CLIENT;
    let timestamp = chrono::Utc::now().timestamp();
    let signature = crypto::generate_signature("heartbeat", timestamp, device_id);
    
    let response = client
        .post(format!("{}/auth/heartbeat", get_api_base()))
        .header("Authorization", format!("Bearer {}", session_token))
        .header("X-Device-ID", device_id)
        .header("X-Timestamp", timestamp.to_string())
        .header("X-Signature", &signature)
        .send()
        .await
        .map_err(|_| "网络连接失败".to_string())?;
    
    if !response.status().is_success() {
        return Ok(HeartbeatResponse { valid: false, expires_at: None });
    }
    
    response.json::<HeartbeatResponse>().await
        .map_err(|_| "数据解析失败".to_string())
}

// 解除设备绑定
pub async fn unbind_device(code: &str, device_id: &str) -> Result<bool, String> {
    let client = &*HTTP_CLIENT;
    let timestamp = chrono::Utc::now().timestamp();
    let signature = crypto::generate_signature(code, timestamp, device_id);
    
    let request = ActivateRequest {
        code: code.to_string(),
        device_id: device_id.to_string(),
        timestamp,
        signature,
    };
    
    let response = client
        .post(format!("{}/auth/unbind", get_api_base()))
        .json(&request)
        .send()
        .await
        .map_err(|_| "网络连接失败".to_string())?;
    
    if !response.status().is_success() {
        return Err("请求失败".to_string());
    }
    
    let resp: serde_json::Value = response.json().await.map_err(|_| "数据解析失败".to_string())?;
    
    if resp.get("success").and_then(|v| v.as_bool()) == Some(true) {
        Ok(true)
    } else {
        Err(resp.get("error").and_then(|v| v.as_str()).unwrap_or("解绑失败").to_string())
    }
}

// 检查服务器上 token 的更新时间
pub async fn check_token_version(
    session_token: &str,
    token_id: &str,
    device_id: &str,
) -> Result<i64, String> {
    let client = &*HTTP_CLIENT;
    let timestamp = chrono::Utc::now().timestamp();
    let signature = crypto::generate_signature(session_token, timestamp, device_id);
    
    let response = client
        .get(format!("{}/tokens/check/{}", get_api_base(), token_id))
        .header("Authorization", format!("Bearer {}", session_token))
        .header("X-Device-ID", device_id)
        .header("X-Timestamp", timestamp.to_string())
        .header("X-Signature", &signature)
        .send()
        .await
        .map_err(|_| "网络连接失败".to_string())?;
    
    if !response.status().is_success() {
        if response.status().as_u16() == 401 {
            return Err("SESSION_EXPIRED".to_string());
        }
        return Err("检查失败".to_string());
    }
    
    let resp: Value = response.json().await.map_err(|_| "数据解析失败".to_string())?;
    
    if resp.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return Err(resp.get("error").and_then(|v| v.as_str()).unwrap_or("未知错误").to_string());
    }
    
    Ok(resp.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(0))
}

// 检查客户端更新
#[derive(Debug, Deserialize)]
pub struct UpdateInfo {
    #[serde(rename = "hasUpdate")]
    pub has_update: bool,
    pub version: Option<String>,
    pub filename: Option<String>,
    pub size: Option<i64>,
    pub changelog: Option<String>,
    #[serde(rename = "forceUpdate")]
    pub force_update: Option<bool>,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
}

pub async fn check_update() -> Result<UpdateInfo, String> {
    let client = &*HTTP_CLIENT;
    
    let response = client
        .get(format!("{}/client/version", get_api_base()))
        .send()
        .await
        .map_err(|_| "网络连接失败".to_string())?;
    
    if !response.status().is_success() {
        return Err("检查更新失败".to_string());
    }
    
    response.json::<UpdateInfo>().await
        .map_err(|_| "数据解析失败".to_string())
}

pub async fn download_update<F>(
    download_url: &str, 
    save_path: &std::path::Path,
    progress_callback: F,
) -> Result<(), String> 
where
    F: Fn(u64, u64) + Send + 'static,
{
    // 为下载创建专用客户端（无总超时，只有连接超时）
    let client = Client::builder()
        .use_rustls_tls()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建下载客户端失败: {}", e))?;
    
    // 构建完整 URL
    let full_url = if download_url.starts_with("http") {
        download_url.to_string()
    } else if download_url.starts_with("/client/") {
        format!("{}{}", get_api_base(), download_url)
    } else {
        format!("{}{}", get_api_base(), download_url)
    };
    
    let response = client
        .get(&full_url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    
    if !response.status().is_success() {
        return Err("下载失败: 服务器错误".to_string());
    }
    
    // 获取文件总大小
    let total_size = response.content_length().unwrap_or(0);
    
    // 创建文件
    let mut file = std::fs::File::create(save_path)
        .map_err(|e| format!("创建文件失败: {}", e))?;
    
    // 流式下载
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("下载中断: {}", e))?;
        file.write_all(&chunk).map_err(|e| format!("写入失败: {}", e))?;
        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }
    
    file.flush().map_err(|e| format!("保存失败: {}", e))?;
    Ok(())
}
