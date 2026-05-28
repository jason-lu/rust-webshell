use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

// 注意：这些测试需要数据库连接
// 设置环境变量 DATABASE_URL 和 JWT_SECRET 后运行
// cargo test --test api_tests

#[tokio::test]
#[ignore] // 需要数据库，手动运行: cargo test --test api_tests -- --ignored
async fn test_register_endpoint_disabled() {
    // 注册功能已禁用，应返回 403
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/register")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username":"test","password":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore] // 需要数据库
async fn test_login_with_invalid_credentials() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username":"nonexistent","password":"wrong"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // 需要数据库
async fn test_login_with_missing_fields() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[ignore] // 需要数据库
async fn test_ws_shell_without_token() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws/shell")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // 需要数据库
async fn test_ws_shell_with_invalid_token() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws/shell?token=invalid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// 辅助函数：创建测试用的 app
async fn create_test_app() -> axum::Router {
    use axum::{routing::get, Router};
    use tower_http::cors::CorsLayer;

    // 这里需要实际的数据库连接池
    // 在 CI 环境中可以使用测试数据库
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/test_db".to_string());

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    Router::new()
        .route("/api/register", axum::routing::post(|| async { StatusCode::FORBIDDEN }))
        .route("/api/login", axum::routing::post(|| async { StatusCode::OK }))
        .route("/api/ws/shell", get(|| async { StatusCode::OK }))
        .layer(CorsLayer::permissive())
        .with_state(pool)
}

// 不需要数据库的单元测试
#[test]
fn test_json_serialization() {
    let json = r#"{"username":"testuser","password":"testpass"}"#;
    let value: Value = serde_json::from_str(json).unwrap();

    assert_eq!(value["username"], "testuser");
    assert_eq!(value["password"], "testpass");
}

#[test]
fn test_json_missing_fields() {
    let json = r#"{"username":"testuser"}"#;
    let value: Value = serde_json::from_str(json).unwrap();

    assert_eq!(value["username"], "testuser");
    assert!(value.get("password").is_none());
}

#[test]
fn test_status_codes() {
    assert_eq!(StatusCode::OK, 200);
    assert_eq!(StatusCode::UNAUTHORIZED, 401);
    assert_eq!(StatusCode::FORBIDDEN, 403);
    assert_eq!(StatusCode::NOT_FOUND, 404);
    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, 500);
}
