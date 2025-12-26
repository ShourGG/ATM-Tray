use aes_gcm::{
    aead::Aead,
    Aes256Gcm, Nonce, KeyInit as AesKeyInit,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use rand::Rng;
use base64::Engine as _;

type HmacSha256 = Hmac<Sha256>;

// 运行时解密字节数组
#[inline(never)]
fn decrypt_static_bytes(encrypted: &[u8], key_seed: u8) -> Vec<u8> {
    let mut result = Vec::with_capacity(encrypted.len());
    let mut key = key_seed;
    
    for (i, &byte) in encrypted.iter().enumerate() {
        let k1 = key.wrapping_mul(0x1B).wrapping_add(i as u8);
        let k2 = k1.rotate_left(3) ^ 0xA5;
        let decrypted = byte ^ k2;
        result.push(decrypted);
        key = key.wrapping_add(decrypted).wrapping_mul(0x3D);
    }
    result
}

// 通信密钥（加密存储）
const ENC_COMM_KEY: [u8; 32] = [
    0x11, 0x18, 0xFC, 0x4A, 0xF9, 0xB8, 0x1A, 0x9C,
    0x4F, 0xA4, 0xD1, 0x44, 0x47, 0xE5, 0x03, 0x57,
    0xA3, 0x2B, 0x8E, 0x38, 0x3B, 0x99, 0x8F, 0x8C,
    0x2B, 0x91, 0x60, 0xC1, 0x15, 0x07, 0x21, 0x53,
];

#[inline(never)]
fn get_encryption_key() -> [u8; 32] {
    let decrypted = decrypt_static_bytes(&ENC_COMM_KEY, 0x8D);
    let mut key = [0u8; 32];
    key.copy_from_slice(&decrypted);
    key
}

pub fn generate_signature(data: &str, timestamp: i64, device_id: &str) -> String {
    let message = format!("{}|{}|{}", data, timestamp, device_id);
    let key = get_encryption_key();
    
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&key).expect("HMAC key error");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

pub fn verify_signature(data: &str, timestamp: i64, device_id: &str, signature: &str) -> bool {
    let expected = generate_signature(data, timestamp, device_id);
    expected == signature
}

const MAX_ENCRYPTED_DATA_LEN: usize = 10 * 1024 * 1024; // 10MB 最大限制，防止 DoS
const AES_GCM_NONCE_LEN: usize = 12;
const AES_GCM_TAG_LEN: usize = 16;

pub fn decrypt_payload(encrypted_data: &str, iv: &str, tag: &str) -> Result<String, String> {
    // 安全校验：防止超大输入导致内存耗尽 (DoS)
    if encrypted_data.len() > MAX_ENCRYPTED_DATA_LEN * 2 
        || iv.len() > 128 
        || tag.len() > 128 
    {
        return Err("Input data too large".to_string());
    }
    
    let key = get_encryption_key();
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&key).map_err(|e| e.to_string())?;
    
    let iv_bytes = hex::decode(iv).map_err(|e| e.to_string())?;
    
    // 安全校验：Nonce 必须是 12 bytes，否则 from_slice 会 panic
    if iv_bytes.len() != AES_GCM_NONCE_LEN {
        return Err(format!("Invalid IV length: expected {}, got {}", AES_GCM_NONCE_LEN, iv_bytes.len()));
    }
    let nonce = Nonce::from_slice(&iv_bytes);
    
    let mut ciphertext = hex::decode(encrypted_data).map_err(|e| e.to_string())?;
    let tag_bytes = hex::decode(tag).map_err(|e| e.to_string())?;
    
    // 安全校验：Tag 必须是 16 bytes
    if tag_bytes.len() != AES_GCM_TAG_LEN {
        return Err(format!("Invalid tag length: expected {}, got {}", AES_GCM_TAG_LEN, tag_bytes.len()));
    }
    ciphertext.extend_from_slice(&tag_bytes);
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| "Decryption failed".to_string())?;
    
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

pub fn encrypt_payload(data: &str) -> Result<(String, String, String), String> {
    let key = get_encryption_key();
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&key).map_err(|e| e.to_string())?;
    
    let mut rng = rand::thread_rng();
    let mut iv_bytes = [0u8; 12];
    rng.fill(&mut iv_bytes);
    let nonce = Nonce::from_slice(&iv_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .map_err(|_| "Encryption failed".to_string())?;
    
    // 分离密文和 tag (最后16字节是tag)
    let (ct, tag) = ciphertext.split_at(ciphertext.len() - 16);
    
    Ok((
        hex::encode(ct),
        hex::encode(iv_bytes),
        hex::encode(tag),
    ))
}

// ==================== 本地存储加密 ====================
// 使用设备指纹派生的密钥加密本地敏感数据

// 本地存储盐值（加密存储）
const SALT_STORAGE: [u8; 26] = [
    0x14, 0x43, 0x95, 0x06, 0xE2, 0xFB, 0x67, 0x54,
    0xBF, 0x43, 0x7F, 0x6B, 0xEF, 0x63, 0x8E, 0xE8,
    0xFE, 0xE7, 0x86, 0xDB, 0xE4, 0x06, 0xDE, 0xF7,
    0xDB, 0x34,
];

// 派生本地存储专用密钥（基于设备指纹）
fn get_local_storage_key() -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let device_fp = get_device_fingerprint();
    let salt = decrypt_static_bytes(&SALT_STORAGE, 0x3A);
    let mut hasher = Sha256::new();
    hasher.update(device_fp.as_bytes());
    hasher.update(&salt);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

// 加密本地存储数据
pub fn encrypt_local_data(plaintext: &str) -> Result<String, String> {
    let key = get_local_storage_key();
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&key).map_err(|e| e.to_string())?;
    
    let mut rng = rand::thread_rng();
    let mut iv_bytes = [0u8; 12];
    rng.fill(&mut iv_bytes);
    let nonce = Nonce::from_slice(&iv_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| "Local encryption failed".to_string())?;
    
    // 格式: base64(iv + ciphertext)
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&iv_bytes);
    combined.extend_from_slice(&ciphertext);
    
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &combined))
}

// 解密本地存储数据
pub fn decrypt_local_data(encrypted: &str) -> Result<String, String> {
    let combined = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encrypted)
        .map_err(|e| e.to_string())?;
    
    if combined.len() < 12 + AES_GCM_TAG_LEN {
        return Err("Encrypted data too short".to_string());
    }
    
    let (iv_bytes, ciphertext) = combined.split_at(12);
    
    let key = get_local_storage_key();
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(iv_bytes);
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Local decryption failed".to_string())?;
    
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

// 设备指纹盐值（加密存储）
const SALT_DEVICE: [u8; 13] = [
    0x41, 0x51, 0x47, 0x90, 0x48, 0xAE, 0xA2, 0x65,
    0x66, 0x5D, 0xD3, 0x9E, 0x56,
];

pub fn get_device_fingerprint() -> String {
    match machine_uid::get() {
        Ok(id) => {
            use sha2::{Sha256, Digest};
            let salt = decrypt_static_bytes(&SALT_DEVICE, 0x5C);
            let mut hasher = Sha256::new();
            hasher.update(id.as_bytes());
            hasher.update(&salt);
            hex::encode(hasher.finalize())[..32].to_string()
        }
        Err(_) => {
            uuid::Uuid::new_v4().to_string().replace("-", "")[..32].to_string()
        }
    }
}

// ==================== 字符串运行时解密 ====================
// 防止静态分析直接看到敏感字符串

#[inline(never)]
pub fn decrypt_static_string(encrypted: &[u8], key_seed: u8) -> String {
    let mut result = Vec::with_capacity(encrypted.len());
    let mut key = key_seed;
    
    for (i, &byte) in encrypted.iter().enumerate() {
        // 多层混淆：XOR + 位旋转 + 索引扰动
        let k1 = key.wrapping_mul(0x1B).wrapping_add(i as u8);
        let k2 = k1.rotate_left(3) ^ 0xA5;
        let decrypted = byte ^ k2;
        result.push(decrypted);
        
        // 密钥滚动
        key = key.wrapping_add(decrypted).wrapping_mul(0x3D);
    }
    
    String::from_utf8(result).unwrap_or_default()
}

// 加密字符串（仅用于生成加密数据，发布时可删除）
#[allow(dead_code)]
pub fn encrypt_static_string(plaintext: &str, key_seed: u8) -> Vec<u8> {
    let mut result = Vec::with_capacity(plaintext.len());
    let mut key = key_seed;
    
    for (i, &byte) in plaintext.as_bytes().iter().enumerate() {
        let k1 = key.wrapping_mul(0x1B).wrapping_add(i as u8);
        let k2 = k1.rotate_left(3) ^ 0xA5;
        let encrypted = byte ^ k2;
        result.push(encrypted);
        key = key.wrapping_add(byte).wrapping_mul(0x3D);
    }
    result
}

// Factory API URL（运行时解密）
pub fn get_factory_api_url() -> String {
    const ENC_FACTORY_API: [u8; 58] = [
        0xC2, 0x64, 0xAA, 0x97, 0xFE, 0x67, 0x78, 0x89,
        0x30, 0x63, 0x57, 0x37, 0x11, 0x2A, 0xAB, 0x2F,
        0xAD, 0x23, 0x14, 0x22, 0xC5, 0xCC, 0x6E, 0xF7,
        0x02, 0xD0, 0x95, 0xC1, 0x88, 0x34, 0xE2, 0x2E,
        0x15, 0x67, 0xF1, 0xE4, 0xF1, 0xD5, 0xA7, 0xA9,
        0x46, 0x96, 0x81, 0xAC, 0x2A, 0xEF, 0xB8, 0x61,
        0x8B, 0xAB, 0x88, 0x5F, 0xC6, 0x6E, 0x8A, 0x6E,
        0xA9, 0xDC,
    ];
    decrypt_static_string(&ENC_FACTORY_API, 0xB3)
}

// API URL 加密数据（运行时解密）
pub fn get_api_url() -> String {
    const ENCRYPTED_URL: [u8; 31] = [
        0xE6, 0x80, 0x49, 0x37, 0x98, 0x01, 0x98, 0x2F,
        0xD4, 0x79, 0x6E, 0xA9, 0x1C, 0x2D, 0x44, 0xA4,
        0x5D, 0x0B, 0xBC, 0x54, 0xD8, 0xF1, 0xAA, 0xEC,
        0x04, 0xB8, 0xEF, 0xEF, 0xCA, 0x06, 0x19,
    ];
    decrypt_static_string(&ENCRYPTED_URL, 0x7F)
}
