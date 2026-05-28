use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// 初始化 PostgreSQL 连接池
/// 
/// # 参数
/// - database_url: PostgreSQL 连接字符串
/// 
/// # 返回
/// 连接池实例，最大连接数为 5
/// 
/// # 错误
/// 连接失败时返回 sqlx::Error
pub async fn init_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)  // 最大连接数
        .connect(database_url)
        .await
}

/// 执行数据库迁移
/// 
/// 创建必要的表结构（如果不存在）：
/// - users: 用户表，包含 id, username, password_hash, created_at, updated_at
/// 
/// # 参数
/// - pool: 数据库连接池
/// 
/// # 错误
/// SQL 执行失败时返回 sqlx::Error
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),  -- 用户 ID，自动生成
            username VARCHAR(64) NOT NULL UNIQUE,           -- 用户名，唯一约束
            password_hash VARCHAR(256) NOT NULL,            -- bcrypt 哈希后的密码
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- 创建时间
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()   -- 更新时间
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}
