use axum::{extract::{Multipart, State}, http::{HeaderMap, StatusCode}, Json};
use futures_util::StreamExt;
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

/// 文件上传处理器
///
/// 验证 JWT token 后，将 multipart 表单中的文件保存到 /root/uploads/
///
/// # 认证
/// 需要在 Authorization 请求头中传递 Bearer token
///
/// # 参数
/// - headers: 请求头（提取 Authorization）
/// - multipart: multipart 表单数据
///
/// # 返回
/// - 成功: "File uploaded: <filename>"
/// - 失败: 401 Unauthorized、400 Bad Request 或 500 Internal Server Error
pub async fn upload_file(
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<MessageResponse>, (StatusCode, Json<MessageResponse>)> {
    // 从 Authorization 头提取 Bearer token
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header"))?;

    // 验证 JWT token
    let config = crate::config::Config::from_env();
    let _claims = auth::verify_token(token, &config.jwt_secret)
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

    // 从 multipart 表单中读取文件字段，流式写入磁盘
    let mut filename: Option<String> = None;
    let mut file_written = false;

    let upload_dir = "/root/uploads";
    tokio::fs::create_dir_all(upload_dir).await.map_err(|e| {
        err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create upload dir: {}", e))
    })?;

    let mut total_bytes: u64 = 0;
    let mut safe_name = String::from("upload");

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        err(StatusCode::BAD_REQUEST, &format!("Failed to read multipart field: {}", e))
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || filename.is_none() {
            filename = field.file_name().map(|s| s.to_string());

            // 防止路径遍历：只保留文件名部分
            if let Some(ref fname) = filename {
                safe_name = std::path::Path::new(fname)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("upload")
                    .to_string();
            }

            // 流式写入文件，避免大文件全部加载到内存
            let path = format!("{}/{}", upload_dir, safe_name);
            let mut file = tokio::fs::File::create(&path).await.map_err(|e| {
                err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create file: {}", e))
            })?;

            let mut stream = field;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    err(StatusCode::BAD_REQUEST, &format!("Failed to read file chunk: {}", e))
                })?;
                use tokio::io::AsyncWriteExt;
                file.write_all(&chunk).await.map_err(|e| {
                    err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to write file: {}", e))
                })?;
                total_bytes += chunk.len() as u64;
            }

            file_written = true;
            if name == "file" {
                break;
            }
        }
    }

    if !file_written {
        return Err(err(StatusCode::BAD_REQUEST, "No file provided"));
    }

    tracing::info!("File uploaded: {} ({} bytes)", safe_name, total_bytes);

    Ok(Json(MessageResponse {
        message: format!("File uploaded: {}", safe_name),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_request_deserialization() {
        let json = r#"{"username":"testuser","password":"testpass"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "testuser");
        assert_eq!(req.password, "testpass");
    }

    #[test]
    fn test_login_request_deserialization() {
        let json = r#"{"username":"admin","password":"123456"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "admin");
        assert_eq!(req.password, "123456");
    }

    #[test]
    fn test_change_password_request_deserialization() {
        let json = r#"{
            "username": "user1",
            "old_password": "old123",
            "new_password": "new456"
        }"#;
        let req: ChangePasswordRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "user1");
        assert_eq!(req.old_password, "old123");
        assert_eq!(req.new_password, "new456");
    }

    #[test]
    fn test_auth_response_serialization() {
        let resp = AuthResponse {
            token: "jwt-token-123".to_string(),
            username: "testuser".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("jwt-token-123"));
        assert!(json.contains("testuser"));
    }

    #[test]
    fn test_message_response_serialization() {
        let resp = MessageResponse {
            message: "Operation successful".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("Operation successful"));
    }

    #[test]
    fn test_err_helper_function() {
        let (status, json_resp) = err(StatusCode::BAD_REQUEST, "Invalid input");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json_resp.message, "Invalid input");
    }

    #[test]
    fn test_missing_fields_in_request() {
        let json = r#"{"username":"test"}"#;
        let result = serde_json::from_str::<LoginRequest>(json);
        assert!(result.is_err());
    }
}
