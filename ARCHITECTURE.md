# ATM-Client 安全架构设计

## 系统概述

```
┌─────────────────┐         ┌─────────────────┐
│   ATM-Client    │◄───────►│   ATM-Server    │
│   (Tauri EXE)   │  HTTPS  │   (后台管理)     │
└─────────────────┘         └─────────────────┘
      客户端                      服务器
```

## 核心功能

### 客户端 (EXE)
- 激活码验证登录
- 显示可用 Token 列表（从服务器获取）
- 切换/激活账号（写入本地 auth.json）
- 查看余额信息
- 无 Token 生成功能（安全考虑）

### 服务器 (后台)
- 激活码管理（生成、分配、禁用）
- Token 池管理（添加、刷新、分配）
- 用户管理
- 使用日志

## 安全防护策略

### 1. 通信安全
- **HTTPS 强制** - 所有 API 通信走 HTTPS
- **请求签名** - 每个请求带时间戳 + HMAC 签名
- **响应加密** - Token 数据 AES-256-GCM 加密传输
- **证书锁定** - 可选，防止中间人攻击

### 2. 客户端安全
- **设备指纹** - 绑定激活码到设备
- **心跳检测** - 定期验证激活状态
- **反调试** - 检测调试器附加
- **完整性校验** - 启动时校验自身哈希
- **内存保护** - 敏感数据加密存储在内存

### 3. 激活码系统
```
激活码结构: XXXX-XXXX-XXXX-XXXX
           ├─ 前缀标识
           ├─ 随机部分 (加密)
           └─ 校验位
```
- 每个激活码对应 N 个 Token 配额
- 支持有效期设置
- 支持设备数限制
- 支持使用次数限制

### 4. 数据安全
- Token 不在客户端持久化（仅运行时）
- 每次启动重新验证
- 敏感配置加密存储

## API 设计

### 认证 API
```
POST /api/v1/auth/activate
Body: {
    "code": "XXXX-XXXX-XXXX-XXXX",  // 激活码
    "device_id": "...",              // 设备指纹
    "timestamp": 1234567890,
    "signature": "..."               // HMAC签名
}
Response: {
    "success": true,
    "session_token": "...",          // 会话令牌
    "expires_at": 1234567890,
    "quota": 5                       // 可用账号数
}
```

### Token 列表 API
```
GET /api/v1/tokens
Headers: {
    "Authorization": "Bearer <session_token>",
    "X-Device-ID": "...",
    "X-Timestamp": "...",
    "X-Signature": "..."
}
Response: {
    "success": true,
    "data": "<encrypted_payload>",   // AES加密的Token列表
    "iv": "...",
    "tag": "..."
}
```

### 切换账号 API
```
POST /api/v1/tokens/activate
Body: {
    "token_id": "...",
    "session_token": "..."
}
Response: {
    "success": true,
    "access_token": "<encrypted>",
    "refresh_token": "<encrypted>"
}
```

## 技术选型

### 客户端
- **框架**: Tauri 2.x (Rust + WebView)
- **加密**: ring / aes-gcm (Rust)
- **HTTP**: reqwest + rustls
- **混淆**: 编译优化 + 符号剥离

### 服务器
- **框架**: Node.js + Express / Rust + Actix-web
- **数据库**: PostgreSQL / SQLite
- **缓存**: Redis (可选)

## 编译安全配置

```toml
[profile.release]
panic = "abort"
codegen-units = 1
lto = true
opt-level = "z"
strip = true
debug = false
```

## 防逆向措施清单

- [x] Release 编译符号剥离
- [x] LTO 链接优化
- [ ] 字符串加密
- [ ] 控制流平坦化 (需要 LLVM 插件)
- [ ] 反调试检测
- [ ] 代码完整性校验
- [ ] API 密钥混淆存储
