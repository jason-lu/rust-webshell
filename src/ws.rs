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
use tokio::sync::mpsc;

use crate::auth;

/// WebSocket 查询参数
#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// 客户端发来的终端 resize 消息
#[derive(Deserialize)]
struct ResizeMsg {
    cols: u16,
    rows: u16,
}

/// PTY 进程信息
struct PtyProcess {
    master_fd: i32,
    child_pid: i32,
}

/// 使用 libc 创建 PTY 并启动 bash（在阻塞线程中运行，避免 tokio 死锁）
fn spawn_pty() -> Result<PtyProcess, String> {
    unsafe {
        let mut master_fd: i32 = -1;
        let mut slave_fd: i32 = -1;
        if libc::openpty(&mut master_fd, &mut slave_fd, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) != 0 {
            return Err(format!("openpty failed: {}", std::io::Error::last_os_error()));
        }

        // 设置初始终端大小
        let ws = libc::winsize { ws_row: 24, ws_col: 80, ws_xpixel: 0, ws_ypixel: 0 };
        libc::ioctl(master_fd, libc::TIOCSWINSZ, &ws);

        // 关闭 fork 前不需要的 cloexec，让子进程继承 slave_fd
        let pid = libc::fork();
        if pid < 0 {
            libc::close(master_fd);
            libc::close(slave_fd);
            return Err(format!("fork failed: {}", std::io::Error::last_os_error()));
        }

        if pid == 0 {
            // === 子进程 ===
            libc::close(master_fd);
            libc::setsid();
            libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);
            libc::dup2(slave_fd, 0);
            libc::dup2(slave_fd, 1);
            libc::dup2(slave_fd, 2);
            if slave_fd > 2 { libc::close(slave_fd); }

            libc::setenv(b"TERM\0".as_ptr().cast(), b"xterm-256color\0".as_ptr().cast(), 1);
            libc::setenv(b"SHELL\0".as_ptr().cast(), b"/bin/bash\0".as_ptr().cast(), 1);
            libc::setenv(b"USER\0".as_ptr().cast(), b"root\0".as_ptr().cast(), 1);
            libc::setenv(b"HOME\0".as_ptr().cast(), b"/root\0".as_ptr().cast(), 1);
            libc::chdir(b"/root\0".as_ptr().cast());
            libc::execlp(b"/bin/bash\0".as_ptr().cast(), b"bash\0".as_ptr().cast(), b"--login\0".as_ptr().cast::<libc::c_char>(), std::ptr::null::<libc::c_char>());
            libc::_exit(127);
        }

        // === 父进程 ===
        libc::close(slave_fd);
        Ok(PtyProcess { master_fd, child_pid: pid })
    }
}

/// 在阻塞线程中同步读取 PTY master
async fn pty_read_loop(master_fd: i32, tx: mpsc::Sender<Bytes>) {
    tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = unsafe { libc::read(master_fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 { break; }
            if tx.blocking_send(Bytes::copy_from_slice(&buf[..n as usize])).is_err() {
                break;
            }
        }
    }).await.ok();
}

/// WebSocket 终端升级处理器
pub async fn ws_shell_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(pool): State<PgPool>,
) -> impl IntoResponse {
    let token = query.token.unwrap_or_default();
    let config = crate::config::Config::from_env();

    let claims = match auth::verify_token(&token, &config.jwt_secret) {
        Some(c) => c,
        None => {
            return axum::http::Response::builder()
                .status(401)
                .body("Unauthorized".into())
                .unwrap();
        }
    };

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

    ws.on_upgrade(move |socket| handle_shell_socket(socket, claims.username))
}

/// 处理 WebSocket 终端会话（支持动态 resize）
async fn handle_shell_socket(socket: WebSocket, username: String) {
    tracing::info!("Shell session started for user: {}", username);

    // 在阻塞线程中创建 PTY 并 fork 子进程（避免 tokio 运行时死锁）
    let pty = match tokio::task::spawn_blocking(spawn_pty).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            tracing::error!("Failed to create PTY: {}", e);
            return;
        }
        Err(e) => {
            tracing::error!("spawn_blocking failed: {}", e);
            return;
        }
    };

    let master_fd = pty.master_fd;
    let child_pid = pty.child_pid;

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Bytes>(64);

    // 任务1: 读取 PTY master -> 通道
    let tx_out = tx.clone();
    let read_task = tokio::spawn(pty_read_loop(master_fd, tx_out));

    // 任务2: 通道 -> WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });

    // 任务3: WebSocket -> PTY master（支持 resize 消息）
    let write_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // 尝试解析为 resize 消息
                    if data.len() > 2 && data[0] == b'{' {
                        if let Ok(text) = std::str::from_utf8(&data) {
                            if let Ok(rm) = serde_json::from_str::<ResizeMsg>(text) {
                                let ws = libc::winsize {
                                    ws_row: rm.rows,
                                    ws_col: rm.cols,
                                    ws_xpixel: 0,
                                    ws_ypixel: 0,
                                };
                                unsafe {
                                    libc::ioctl(master_fd, libc::TIOCSWINSZ, &ws);
                                    libc::kill(child_pid, libc::SIGWINCH);
                                }
                                continue;
                            }
                        }
                    }
                    unsafe { libc::write(master_fd, data.as_ptr().cast(), data.len()); }
                }
                Ok(Message::Text(text)) => {
                    unsafe { libc::write(master_fd, text.as_ptr().cast(), text.len()); }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    // 等待任意任务结束
    tokio::select! {
        _ = read_task => {},
        _ = send_task => {},
        _ = write_task => {},
    }

    // 清理
    unsafe {
        libc::kill(child_pid, libc::SIGTERM);
        libc::waitpid(child_pid, std::ptr::null_mut(), 0);
        libc::close(master_fd);
    }
    tracing::info!("Shell session ended for user: {}", username);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_query_deserialization_with_token() {
        let query = WsQuery {
            token: Some("my-token".to_string()),
        };
        assert_eq!(query.token.unwrap(), "my-token");
    }

    #[test]
    fn test_ws_query_deserialization_without_token() {
        let query = WsQuery { token: None };
        assert!(query.token.is_none());
    }

    #[test]
    fn test_ws_query_from_json() {
        let json = r#"{"token":"***"}"#;
        let query: WsQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.token.unwrap(), "abc123");
    }

    #[test]
    fn test_ws_query_from_empty_json() {
        let json = r#"{}"#;
        let query: WsQuery = serde_json::from_str(json).unwrap();
        assert!(query.token.is_none());
    }
}
