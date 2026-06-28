use axum::{extract::{Request, State, Query}, http::{HeaderMap, StatusCode}, Json, body::Body, response::Response};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth;

// ==================== 请求/响应结构体 ====================

/// 注册请求体
#[derive(Deserialize)]
#[allow(dead_code)]
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
    request: Request<Body>,
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

    // 从 Content-Type 提取 boundary
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let content_length = headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    tracing::info!("Upload: content_type={}, content_length={}", content_type, content_length);

    // 读取完整请求体
    let body_bytes = axum::body::to_bytes(request.into_body(), 100 * 1024 * 1024).await.map_err(|e| {
        tracing::error!("Body read failed: {}", e);
        err(StatusCode::BAD_REQUEST, &format!("Failed to read request body: {}", e))
    })?;
    tracing::info!("Body received: {} bytes", body_bytes.len());

    // 手动解析 multipart/form-data
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .unwrap_or("")
        .trim_matches('"')
        .to_string();

    if boundary.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Missing boundary in Content-Type"));
    }

    let boundary_delim = format!("--{}", boundary);
    let body_str = String::from_utf8_lossy(&body_bytes);

    // 找到文件部分：在 boundary 之间查找 filename
    let upload_dir = "/root/uploads";
    tokio::fs::create_dir_all(upload_dir).await.map_err(|e| {
        err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to create upload dir: {}", e))
    })?;

    let mut safe_name = String::from("upload");
    let mut file_data: Option<&[u8]> = None;

    // 按 boundary 分割
    let parts: Vec<&str> = body_str.split(&boundary_delim).collect();
    for part in &parts[1..] {
        if part.starts_with("--") { continue; } // 结束标记

        // 分离 headers 和 body（用 \r\n\r\n 分隔）
        if let Some(header_end) = part.find("\r\n\r\n") {
            let headers_str = &part[..header_end];
            let body_part = &part[header_end + 4..];

            // 查找 filename
            if let Some(fn_start) = headers_str.find("filename=\"") {
                let fn_rest = &headers_str[fn_start + 10..];
                if let Some(fn_end) = fn_rest.find('\"') {
                    let raw_filename = &fn_rest[..fn_end];
                    safe_name = std::path::Path::new(raw_filename)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("upload")
                        .to_string();
                }
            }

            // 去掉末尾的 \r\n
            let body_bytes_part = body_part.as_bytes();
            let body_trimmed = if body_bytes_part.ends_with(b"\r\n") {
                &body_bytes_part[..body_bytes_part.len() - 2]
            } else {
                body_bytes_part
            };
            file_data = Some(body_trimmed);
            break;
        }
    }

    let data = file_data.ok_or_else(|| err(StatusCode::BAD_REQUEST, "No file found in multipart data"))?;
    let path = format!("{}/{}", upload_dir, safe_name);
    tokio::fs::write(&path, data).await.map_err(|e| {
        err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to write file: {}", e))
    })?;

    tracing::info!("File uploaded: {} ({} bytes)", safe_name, data.len());

    Ok(Json(MessageResponse {
        message: format!("File uploaded: {}", safe_name),
    }))
}

/// 文件列表响应体
#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,     // 文件名
    pub size: u64,        // 文件大小（字节）
    pub is_dir: bool,     // 是否为目录
}

/// 文件列表查询参数
#[derive(Deserialize)]
pub struct ListQuery {
    pub path: Option<String>, // 可选子目录路径
}

/// 文件列表处理器
///
/// 列出 /root/uploads/ 下的文件和目录
pub async fn list_files(
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<FileEntry>>, (StatusCode, Json<MessageResponse>)> {
    // 认证
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header"))?;

    let config = crate::config::Config::from_env();
    let _claims = auth::verify_token(token, &config.jwt_secret)
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

    let sub_path = query.path.unwrap_or_default();
    if sub_path.contains("..") || sub_path.contains('\0') {
        return Err(err(StatusCode::FORBIDDEN, "Invalid path"));
    }

    let dir_path = PathBuf::from("/root/uploads").join(&sub_path);
    let canonical = dir_path.canonicalize().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            err(StatusCode::NOT_FOUND, "Directory not found")
        } else {
            err(StatusCode::BAD_REQUEST, &format!("Invalid path: {}", e))
        }
    })?;

    let upload_dir = PathBuf::from("/root/uploads");
    if !canonical.starts_with(&upload_dir) {
        return Err(err(StatusCode::FORBIDDEN, "Access denied"));
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&canonical).await.map_err(|e| {
        err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to read directory: {}", e))
    })?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| {
        err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to read entry: {}", e))
    })? {
        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        entries.push(FileEntry {
            name,
            size: metadata.len(),
            is_dir: metadata.is_dir(),
        });
    }

    // 目录在前，文件在后，各自按名称排序
    entries.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else if a.is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    Ok(Json(entries))
}

/// 文件下载查询参数
#[derive(Deserialize)]
pub struct DownloadQuery {
    pub path: String,  // 文件路径（相对于 /root/uploads/）
    pub token: Option<String>, // 可选的 JWT token（用于 <a> 标签下载）
}

/// 文件下载处理器
///
/// 验证 JWT token 后，返回指定文件供浏览器下载。
/// 支持两种认证方式：Authorization header 或 ?token= 查询参数。
///
/// # 参数
/// - headers: 请求头（提取 Authorization）
/// - query: 查询参数，包含 path 和可选 token
///
/// # 返回
/// - 成功: 文件流（带 Content-Disposition: attachment）
/// - 失败: 401 Unauthorized、400 Bad Request、403 Forbidden 或 404 Not Found
pub async fn download_file(
    headers: HeaderMap,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, (StatusCode, Json<MessageResponse>)> {
    // 从 Authorization 头或查询参数提取 token
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or(query.token)
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header"))?;

    // 验证 JWT token
    let config = crate::config::Config::from_env();
    let _claims = auth::verify_token(&token, &config.jwt_secret)
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

    // 安全检查：禁止路径穿越
    if query.path.contains("..") || query.path.contains('\0') {
        return Err(err(StatusCode::FORBIDDEN, "Invalid path"));
    }

    // 拼接完整路径
    let upload_dir = PathBuf::from("/root/uploads");
    let file_path = upload_dir.join(&query.path);

    // 确保路径仍在上传目录内
    let canonical = file_path.canonicalize().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            err(StatusCode::NOT_FOUND, "File not found")
        } else {
            err(StatusCode::BAD_REQUEST, &format!("Invalid path: {}", e))
        }
    })?;
    if !canonical.starts_with(&upload_dir) {
        return Err(err(StatusCode::FORBIDDEN, "Access denied"));
    }

    // 读取文件
    let data = tokio::fs::read(&canonical).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            err(StatusCode::NOT_FOUND, "File not found")
        } else {
            err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to read file: {}", e))
        }
    })?;

    // 提取文件名
    let filename = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    // 推断 MIME 类型
    let mime = mime_guess::from_path(&canonical)
        .first_or_octet_stream()
        .to_string();

    tracing::info!("Download: {} ({} bytes, {})", filename, data.len(), mime);

    // 构造响应
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime)
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        )
        .header("Content-Length", data.len())
        .body(Body::from(data))
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to build response: {}", e)))?;

    Ok(response)
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
