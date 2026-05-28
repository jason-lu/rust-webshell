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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_with_defaults() {
        // 清除环境变量，确保使用默认值
        std::env::remove_var("PORT");

        // 设置必填的环境变量
        std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        std::env::set_var("JWT_SECRET", "test-secret");

        let config = Config::from_env();
        assert_eq!(config.database_url, "postgres://test:test@localhost/test");
        assert_eq!(config.jwt_secret, "test-secret");
        assert_eq!(config.port, 3000);
    }

    #[test]
    fn test_config_from_env_with_custom_port() {
        std::env::set_var("DATABASE_URL", "postgres://localhost/db");
        std::env::set_var("JWT_SECRET", "secret");
        std::env::set_var("PORT", "8080");

        let config = Config::from_env();
        assert_eq!(config.port, 8080);

        std::env::remove_var("PORT");
    }

    #[test]
    fn test_config_from_env_with_invalid_port() {
        std::env::set_var("DATABASE_URL", "postgres://localhost/db");
        std::env::set_var("JWT_SECRET", "secret");
        std::env::set_var("PORT", "invalid");

        let config = Config::from_env();
        assert_eq!(config.port, 3000); // 应该回退到默认值

        std::env::remove_var("PORT");
    }

    #[test]
    #[should_panic(expected = "DATABASE_URL environment variable is required")]
    fn test_config_panics_without_database_url() {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("JWT_SECRET");
        Config::from_env();
    }
}
