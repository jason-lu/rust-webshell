use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use sqlx::PgPool;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::auth;

/// WebSocket 查询参数
/// 
/// 通过 URL query string 传递 JWT token 进行认证
/// 例如: ws://host/api/ws/shell?token=xxx
#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>, // JWT token
}

/// WebSocket 终端升级处理器
/// 
/// 验证 token 后将 HTTP 连接升级为 WebSocket
/// 
/// # 参数
/// - ws: WebSocket 升级器
/// - query: URL 查询参数（包含 token）
/// - pool: 数据库连接池
/// 
/// # 返回
/// - 成功: WebSocket 升级响应
/// - 失败: 401 Unauthorized
pub async fn ws_shell_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(pool): State<PgPool>,
) -> impl IntoResponse {
    let token = query.token.unwrap_or_default();
    let config = crate::config::Config::from_env();

    // 验证 JWT token
    let claims = match auth::verify_token(&token, &config.jwt_secret) {
        Some(c) => c,
        None => {
            return axum::http::Response::builder()
                .status(401)
                .body("Unauthorized".into())
                .unwrap();
        }
    };

    // 检查用户是否存在于数据库
    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1::uuid)",
    )
    .bind(&claims.sub)
    .fetch_one(&pool)
    .await
    .unwrap_or((false,));

    if !exists.0 {
        return axum::http::Response::builder()
            .status(401)
            .body("User not found".into())
            .unwrap();
    }

    // 升级为 WebSocket 连接
    ws.on_upgrade(move |socket| handle_shell_socket(socket, claims.username))
}

/// 处理 WebSocket 终端会话
/// 
/// 创建一个 bash 子进程，将 WebSocket 与子进程的 stdin/stdout/stderr 双向桥接：
/// 
/// 数据流：
///   客户端 → WebSocket → stdin → bash 子进程
///   bash 子进程 → stdout/stderr → WebSocket → 客户端
/// 
/// 使用 `script` 命令包装 bash，以获得正确的终端行为（如颜色、行编辑等）
/// 
/// # 参数
/// - socket: WebSocket 连接
/// - username: 用户名（用于日志）
async fn handle_shell_socket(socket: WebSocket, username: String) {
    tracing::info!("Shell session started for user: {}", username);

    // 使用 script 命令创建伪终端（PTY），-q 静默，-f 立即刷新，-c 指定命令
    let mut cmd = Command::new("script");
    cmd.arg("-qfc")
        .arg("/bin/bash --login")  // 启动登录 shell
        .arg("/dev/null")          // 不保存 script 输出
        .env("TERM", "xterm-256color")  // 终端类型
        .env("SHELL", "/bin/bash")
        .env("USER", "root")
        .env("HOME", "/root")
        .current_dir("/root")      // 初始工作目录
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // 启动子进程
    let mut child = cmd.spawn().expect("Failed to spawn shell");

    // 获取子进程的 stdin/stdout/stderr 句柄
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // 拆分 WebSocket 为发送端和接收端
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    // stdin 需要被多个任务共享，使用 Arc<Mutex> 包装
    let stdin = std::sync::Arc::new(tokio::sync::Mutex::new(stdin));

    // 创建通道，将 stdout 和 stderr 合并到一个发送端
    let (tx, mut rx) = mpsc::channel::<Bytes>(64);

    // 任务1: 读取子进程 stdout，发送到通道
    let tx_stdout = tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf = vec![0u8; 4096]; // 4KB 缓冲区
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    // 发送到通道，失败则退出
                    if tx_stdout.send(Bytes::copy_from_slice(&buf[..n])).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 任务2: 读取子进程 stderr，发送到通道
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = vec![0u8, 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(Bytes::copy_from_slice(&buf[..n])).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 任务3: 从通道读取数据，发送到 WebSocket
    let ws_send_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });

    // 任务4: 从 WebSocket 读取消息，写入子进程 stdin
    let stdin_task = {
        let stdin = stdin.clone();
        tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        let mut stdin = stdin.lock().await;
                        if stdin.write_all(&data).await.is_err() {
                            break;
                        }
                        let _ = stdin.flush().await;
                    }
                    Ok(Message::Text(text)) => {
                        let mut stdin = stdin.lock().await;
                        if stdin.write_all(text.as_bytes()).await.is_err() {
                            break;
                        }
                        let _ = stdin.flush().await;
                    }
                    Ok(Message::Close(_)) => break, // 客户端关闭连接
                    Err(_) => break,
                    _ => {} // 忽略其他消息类型
                }
            }
        })
    };

    // 等待任意一个任务结束（通常是客户端断开或子进程退出）
    tokio::select! {
        _ = stdout_task => {},
        _ = stderr_task => {},
        _ = ws_send_task => {},
        _ = stdin_task => {},
    }

    // 杀死子进程，清理资源
    let _ = child.kill().await;
    tracing::info!("Shell session ended for user: {}", username);
}
