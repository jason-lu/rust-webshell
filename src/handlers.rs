use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth;

// ==================== 请求/响应结构体 ====================

/// 注册请求体
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String, // 用户名
    pub password: String, // 密码（明文，服务端会哈希）
}

/// 登录请求体
#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String, // 用户名
    pub password: String, // 密码
}

/// 修改密码请求体
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub username: String,     // 用户名
    pub old_password: String, // 旧密码
    pub new_password: String, // 新密码
}

/// 认证成功响应体
#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,    // JWT token
    pub username: String, // 用户名
}

/// 通用消息响应体
#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String, // 消息内容
}

/// 构造错误响应的辅助函数
/// 
/// # 参数
/// - status: HTTP 状态码
/// - msg: 错误消息
/// 
/// # 返回
/// (StatusCode, Json<MessageResponse>) 格式的错误响应
fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<MessageResponse>) {
    (status, Json(MessageResponse { message: msg.to_string() }))
}

// ==================== 处理器函数 ====================

/// 用户注册处理器
/// 
/// 当前已禁用，返回 403
/// 
/// # 参数
/// - _state: 数据库连接池（未使用）
/// - _req: 注册请求体（未使用）
/// 
/// # 返回
/// 始终返回 403 FORBIDDEN
pub async fn register(
    _state: State<PgPool>,
    _req: Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<MessageResponse>)> {
    // 注册功能已禁用，只能通过数据库手动添加用户
    Err(err(StatusCode::FORBIDDEN, "Registration is disabled"))
}

/// 用户登录处理器
/// 
/// 验证用户名和密码，成功返回 JWT token
/// 
/// # 参数
/// - pool: 数据库连接池
/// - req: 登录请求体
/// 
/// # 返回
/// - 成功: AuthResponse（包含 token 和 username）
/// - 失败: 401 Unauthorized 或 500 Internal Server Error
pub async fn login(
    State(pool): State<PgPool>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<MessageResponse>)> {
    // 从数据库查询用户
    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, username, password_hash FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_optional(&pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // 用户不存在
    let (id, username, password_hash) =
        row.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    // 验证密码（bcrypt）
    let valid = bcrypt::verify(&req.password, &password_hash)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    if !valid {
        return Err(err(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    // 生成 JWT token
    let config = crate::config::Config::from_env();
    let token = auth::create_token(&id.to_string(), &username, &config.jwt_secret);

    Ok(Json(AuthResponse { token, username }))
}

/// 修改密码处理器
/// 
/// 验证旧密码后更新为新密码
/// 
/// # 参数
/// - pool: 数据库连接池
/// - req: 修改密码请求体
/// 
/// # 返回
/// - 成功: "Password changed successfully"
/// - 失败: 404 Not Found、401 Unauthorized 或 500 Internal Server Error
pub async fn change_password(
    State(pool): State<PgPool>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<MessageResponse>, (StatusCode, Json<MessageResponse>)> {
    // 查询用户的密码哈希
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT password_hash FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_optional(&pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // 用户不存在
    let (password_hash,) =
        row.ok_or_else(|| err(StatusCode::NOT_FOUND, "User not found"))?;

    // 验证旧密码
    let valid = bcrypt::verify(&req.old_password, &password_hash)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    if !valid {
        return Err(err(StatusCode::UNAUTHORIZED, "Old password is incorrect"));
    }

    // 哈希新密码（bcrypt cost=10）
    let new_hash = bcrypt::hash(&req.new_password, 10)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // 更新数据库中的密码哈希
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE username = $2")
        .bind(&new_hash)
        .bind(&req.username)
        .execute(&pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(MessageResponse {
        message: "Password changed successfully".into(),
    }))
}
