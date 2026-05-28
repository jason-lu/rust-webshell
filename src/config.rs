/// 应用配置结构体
/// 
/// 从环境变量读取，支持默认值：
/// - DATABASE_URL: PostgreSQL 连接地址
/// - JWT_SECRET: JWT 签名密钥
/// - PORT: 服务监听端口
pub struct Config {
    pub database_url: String, // PostgreSQL 连接字符串
    pub jwt_secret: String,   // JWT 签名密钥
    pub port: u16,            // HTTP 服务端口
}

impl Config {
    /// 从环境变量加载配置
    /// 
    /// 环境变量未设置时使用默认值：
    /// - DATABASE_URL → 必须设置，无默认值
    /// - JWT_SECRET → 必须设置，无默认值
    /// - PORT → 3000
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL environment variable is required"),
            jwt_secret: std::env::var("JWT_SECRET")
                .expect("JWT_SECRET environment variable is required"),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
        }
    }
}
