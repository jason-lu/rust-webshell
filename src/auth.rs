use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT Claims 结构体
/// 
/// 包含在 JWT token 中的用户信息：
/// - sub: 用户 ID（UUID 字符串）
/// - username: 用户名
/// - exp: 过期时间戳（Unix 时间戳，秒）
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // 用户 ID
    pub username: String, // 用户名
    pub exp: usize,       // 过期时间
}

/// 创建 JWT token
/// 
/// # 参数
/// - user_id: 用户 UUID
/// - username: 用户名
/// - secret: JWT 签名密钥
/// 
/// # 返回
/// 签名后的 JWT 字符串，有效期 24 小时
pub fn create_token(user_id: &str, username: &str, secret: &str) -> String {
    // 计算过期时间：当前时间 + 24小时
    let exp = (Utc::now() + Duration::hours(24)).timestamp() as usize;
    
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp,
    };
    
    // 使用 HS256 算法签名
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

/// 验证 JWT token
/// 
/// # 参数
/// - token: JWT 字符串
/// - secret: JWT 签名密钥
/// 
/// # 返回
/// - Some(Claims): 验证成功，返回解析出的 claims
/// - None: 验证失败（过期、签名错误等）
pub fn verify_token(token: &str, secret: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|data| data.claims)
}
