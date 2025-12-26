# ATM-Client

安全版 Token 管理客户端 - 使用激活码系统

## 架构

```
┌─────────────────┐         ┌─────────────────┐
│   ATM-Client    │◄───────►│   ATM-Server    │
│   (Tauri EXE)   │  HTTPS  │   (后台管理)     │
└─────────────────┘         └─────────────────┘
```

## 功能特性

### 客户端 (EXE)
- ✅ 激活码验证登录
- ✅ 设备指纹绑定
- ✅ 加密通信 (AES-256-GCM)
- ✅ 请求签名验证 (HMAC-SHA256)
- ✅ 反调试检测
- ✅ Token 列表展示
- ✅ 一键激活账号
- ✅ 余额查询

### 服务器 (后台)
- ✅ 激活码生成/管理
- ✅ Token 池管理
- ✅ Token 分配给激活码
- ✅ 会话管理
- ✅ 使用日志
- ✅ 统计信息

## 安全措施

1. **通信安全**: HTTPS + AES-256-GCM 加密 + HMAC 签名
2. **设备绑定**: 激活码绑定到首次使用的设备
3. **反调试**: Windows API 检测调试器
4. **时间验证**: 请求时间戳 5 分钟有效
5. **会话管理**: 24 小时自动过期
6. **编译保护**: LTO + 符号剥离 + 体积优化

## 快速开始

### 1. 配置服务器

```bash
cd server
cp .env.example .env
# 编辑 .env 修改密钥

npm install
npm start
```

### 2. 修改客户端 API 地址

编辑 `src-tauri/src/api.rs`:
```rust
const API_BASE: &str = "https://your-server.com/api/v1";
```

### 3. 编译客户端

```bash
npm install
npm run build
```

## 完整使用流程

### 1️⃣ 管理员：添加 Token 到服务器
```bash
POST /api/v1/admin/tokens
Authorization: Bearer <ADMIN_KEY>

{
  "email": "user1@example.com",
  "access_token": "eyJhbG...",
  "refresh_token": "eyJhbG..."
}
```

### 2️⃣ 管理员：生成激活码并绑定 Token
```bash
POST /api/v1/admin/licenses
Authorization: Bearer <ADMIN_KEY>

{
  "token_ids": ["token-id-1", "token-id-2"],  # 绑定2个Token
  "expires_days": 30,                          # 30天有效期
  "note": "客户A"
}

# 返回
{
  "success": true,
  "licenses": [{
    "code": "ABCD-EFGH-IJKL-MNOP",  # 激活码
    "quota": 2,                      # 包含2个Token
    "bound_tokens": ["token-id-1", "token-id-2"]
  }]
}
```

### 3️⃣ 用户：输入激活码
- 打开客户端 EXE
- 输入激活码 `ABCD-EFGH-IJKL-MNOP`
- 显示绑定的 2 个 Token 账号

### 4️⃣ 用户：选择账号激活
- 点击某个账号的「激活」按钮
- Token 写入本地 `~/.factory/auth.json`
- 即可使用 Droid/Cursor

---

## API 文档

### 管理接口 (需要 Admin Key)

```bash
# 设置 Header
Authorization: Bearer <ADMIN_KEY>
```

#### 激活码管理

```bash
# 生成激活码 (直接绑定Token)
POST /api/v1/admin/licenses
{
  "token_ids": ["id1", "id2"],  # 要绑定的Token ID列表
  "expires_days": 30,           # 有效天数 (可选)
  "note": "备注"
}

# 获取激活码列表
GET /api/v1/admin/licenses

# 禁用/启用激活码
PATCH /api/v1/admin/licenses/:id
{ "is_active": false }

# 删除激活码
DELETE /api/v1/admin/licenses/:id
```

#### Token 池管理

```bash
# 添加 Token
POST /api/v1/admin/tokens
{
  "email": "user@example.com",
  "access_token": "...",
  "refresh_token": "..."
}

# 获取 Token 列表
GET /api/v1/admin/tokens

# 更新 Token
PATCH /api/v1/admin/tokens/:id
{ "is_valid": false }

# 删除 Token
DELETE /api/v1/admin/tokens/:id
```

#### Token 分配

```bash
# 分配 Token 给激活码
POST /api/v1/admin/assign
{
  "license_id": "xxx",
  "token_ids": ["token1", "token2"]
}

# 取消分配
DELETE /api/v1/admin/assign/:license_id/:token_id
```

#### 统计与日志

```bash
# 获取统计
GET /api/v1/admin/stats

# 获取日志
GET /api/v1/admin/logs?limit=100&offset=0
```

## 目录结构

```
ATM-Client/
├── src/                    # 前端
│   ├── index.html
│   ├── styles.css
│   └── renderer.js
│
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # 入口
│   │   ├── commands.rs     # Tauri 命令
│   │   ├── api.rs          # HTTP API
│   │   ├── crypto.rs       # 加密模块
│   │   ├── security.rs     # 安全检测
│   │   └── storage.rs      # 存储
│   └── Cargo.toml
│
├── server/                 # Node.js 服务器
│   ├── src/
│   │   ├── index.js        # 入口
│   │   ├── db.js           # 数据库
│   │   ├── routes/         # 路由
│   │   └── middleware/     # 中间件
│   └── package.json
│
└── ARCHITECTURE.md         # 架构设计
```

## 部署注意事项

1. **修改加密密钥**: 客户端和服务器的密钥必须一致
2. **使用 HTTPS**: 生产环境必须使用 HTTPS
3. **设置强 Admin Key**: 后台管理密钥要足够复杂
4. **定期刷新 Token**: 服务器应实现 Token 自动刷新
5. **备份数据库**: 定期备份 SQLite 数据库

## License

MIT
