# Rust WebShell

基于 Rust + Axum 的 Web 终端，支持用户注册/登录，通过浏览器访问服务器 Shell。

## 技术栈

- **Axum** — Web 框架
- **sqlx** — PostgreSQL 异步驱动
- **bcrypt** — 密码加密（自动加盐）
- **JWT** — 身份认证（24小时过期）
- **WebSocket + xterm.js** — 浏览器终端

## 环境变量

| 变量 | 说明 | 必填 |
|------|------|------|
| `DATABASE_URL` | PostgreSQL 连接串 | ✅ |
| `JWT_SECRET` | JWT 签名密钥 | ✅ |
| `PORT` | 服务端口 | 否（默认 3000） |

## 运行

```bash
# 编译
cargo build --release

# 运行
./target/release/rust-webshell
```

或使用环境变量：

```bash
JWT_SECRET=my-secret-key PORT=8080 ./target/release/rust-webshell
```

## API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/register` | POST | 注册 `{username, password}` |
| `/api/login` | POST | 登录 `{username, password}` → 返回 JWT |
| `/api/change-password` | POST | 改密码 `{username, old_password, new_password}` |
| `/api/ws/shell?token=xxx` | WebSocket | 连接 Shell 终端 |

## 前端

访问 `http://localhost:3000` 即可使用 Web 终端界面。
