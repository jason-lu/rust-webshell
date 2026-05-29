// 模块声明
mod auth;      // JWT 认证模块
mod config;    // 配置管理模块
mod db;        // 数据库操作模块
mod handlers;  // HTTP 请求处理器模块
mod ws;        // WebSocket 终端模块

use axum::{
    Router,
    routing::{get, post},
};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing_subscriber;

/// 程序入口
/// 
/// 启动流程：
/// 1. 初始化日志系统
/// 2. 从环境变量加载配置
/// 3. 连接数据库并执行迁移
/// 4. 注册路由并启动 HTTP 服务
#[tokio::main]
async fn main() {
    // 初始化 tracing 日志，输出到 stdout
    tracing_subscriber::fmt::init();

    // 从环境变量读取配置（数据库地址、JWT密钥、端口等）
    let cfg = config::Config::from_env();

    // 初始化 PostgreSQL 连接池，失败则 panic
    let pool = db::init_pool(&cfg.database_url).await.expect("Failed to connect to database");

    // 执行数据库迁移（创建 users 表等）
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    // 构建 Axum 路由
    let app = Router::new()
        // 用户认证相关接口
        .route("/api/register", post(handlers::register))          // 注册（当前已禁用）
        .route("/api/login", post(handlers::login))                // 登录，返回 JWT
        .route("/api/change-password", post(handlers::change_password))  // 修改密码
        .route("/api/upload", post(handlers::upload_file))              // 文件上传
        // WebSocket 终端接口
        .route("/api/ws/shell", get(ws::ws_shell_handler))
        // 静态文件服务（前端页面），作为兜底路由
        .fallback_service(ServeDir::new("/usr/local/share/webshell/static"))
        // 允许跨域（开发时方便前端调用）
        .layer(CorsLayer::permissive())
        // 注入数据库连接池作为共享状态
        .with_state(pool);

    // 绑定地址并启动服务
    let addr = format!("127.0.0.1:{}", cfg.port);
    tracing::info!("Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
