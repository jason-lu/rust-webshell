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

/// 处理 WebSocket 终端会话（使用 libc PTY，支持动态 resize）
async fn handle_shell_socket(socket: WebSocket, username: String) {
    tracing::info!("Shell session started for user: {}", username);

    // 创建 PTY pair
    let mut master_fd: i32 = -1;
    let mut slave_fd: i32 = -1;
    let ret = unsafe { libc::openpty(&mut master_fd, &mut slave_fd, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
    if ret != 0 {
        tracing::error!("Failed to open PTY: {}", std::io::Error::last_os_error());
        return;
    }

    // 设置初始终端大小
    let mut winsize = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(master_fd, libc::TIOCSWINSZ, &mut winsize); }

    // Fork 子进程
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        tracing::error!("Failed to fork: {}", std::io::Error::last_os_error());
        unsafe { libc::close(master_fd); libc::close(slave_fd); }
        return;
    }

    if pid == 0 {
        // === 子进程 ===
        unsafe {
            libc::close(master_fd);

            // 创建新会话
            libc::setsid();

            // 设置 slave 为控制终端
            libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);

            // 重定向 stdin/stdout/stderr 到 slave
            libc::dup2(slave_fd, 0);
            libc::dup2(slave_fd, 1);
            libc::dup2(slave_fd, 2);
            if slave_fd > 2 {
                libc::close(slave_fd);
            }

            // 设置环境变量
            libc::setenv(b"TERM\0".as_ptr().cast(), b"xterm-256color\0".as_ptr().cast(), 1);
            libc::setenv(b"SHELL\0".as_ptr().cast(), b"/bin/bash\0".as_ptr().cast(), 1);
            libc::setenv(b"USER\0".as_ptr().cast(), b"root\0".as_ptr().cast(), 1);
            libc::setenv(b"HOME\0".as_ptr().cast(), b"/root\0".as_ptr().cast(), 1);

            libc::chdir(b"/root\0".as_ptr().cast());

            // 启动 bash
            libc::execlp(b"/bin/bash\0".as_ptr().cast(), b"bash\0".as_ptr().cast(), b"--login\0".as_ptr().cast::<libc::c_char>(), std::ptr::null::<libc::c_char>());
            libc::_exit(127);
        }
    }

    // === 父进程 ===
    unsafe { libc::close(slave_fd); }

    // 包装 master fd 为 OwnedFd（用于自动关闭）
    // 不需要，我们手动管理
    let master_raw = master_fd;
    let child_pid = pid;

    // 拆分 WebSocket
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Bytes>(64);

    // 任务1: 读取 PTY master -> 通道
    let tx_out = tx.clone();
    let stdout_task = tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = unsafe { libc::read(master_raw, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 { break; }
            if tx_out.blocking_send(Bytes::copy_from_slice(&buf[..n as usize])).is_err() {
                break;
            }
        }
    });

    // 任务2: 通道 -> WebSocket
    let ws_send_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });

    // 任务3: WebSocket -> PTY master（支持 resize 消息）
    let stdin_task = {
        let master_raw = master_raw;
        tokio::spawn(async move {
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
                                        libc::ioctl(master_raw, libc::TIOCSWINSZ, &ws);
                                        libc::kill(child_pid, libc::SIGWINCH);
                                    }
                                    continue;
                                }
                            }
                        }
                        unsafe { libc::write(master_raw, data.as_ptr().cast(), data.len()); }
                    }
                    Ok(Message::Text(text)) => {
                        unsafe { libc::write(master_raw, text.as_ptr().cast(), text.len()); }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        })
    };

    // 等待任意任务结束
    tokio::select! {
        _ = stdout_task => {},
        _ = ws_send_task => {},
        _ = stdin_task => {},
    }

    // 清理
    unsafe {
        libc::kill(child_pid, libc::SIGTERM);
        libc::waitpid(child_pid, std::ptr::null_mut(), 0);
        libc::close(master_raw);
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
        let json = r#"{"token":"abc123"}"#;
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
