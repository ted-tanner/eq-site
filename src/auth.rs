use actix_web::cookie::{Cookie, SameSite};
use argon2_kdf::{Algorithm, Hasher};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as b64_urlsafe;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;

use crate::env;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthTokenType {
    Access,
    Refresh,
    SignIn,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    #[serde(rename = "uid")]
    pub user_id: String,
    #[serde(rename = "tv")]
    pub token_version: i32,
    #[serde(rename = "st")]
    pub account_status: String,
    #[serde(rename = "exp")]
    pub expiration: u64,
    #[serde(rename = "typ")]
    pub token_type: AuthTokenType,
}

#[derive(Debug)]
pub enum AuthError {
    TokenTooLong,
    InvalidTokenEncoding,
    InvalidTokenFormat,
    InvalidSignature,
    InvalidClaims,
    WrongTokenType,
    TokenExpired,
    InvalidHmacKey,
    Hashing(String),
    InvalidHashFormat(String),
    BackgroundTaskFailed,
    ClaimSerialization(serde_json::Error),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenTooLong => write!(f, "Token too long"),
            Self::InvalidTokenEncoding => write!(f, "Invalid token encoding"),
            Self::InvalidTokenFormat => write!(f, "Invalid token format"),
            Self::InvalidSignature => write!(f, "Invalid token signature"),
            Self::InvalidClaims => write!(f, "Invalid token claims"),
            Self::WrongTokenType => write!(f, "Wrong token type"),
            Self::TokenExpired => write!(f, "Token expired"),
            Self::InvalidHmacKey => write!(f, "Invalid HMAC key"),
            Self::Hashing(message) => write!(f, "Hashing failed: {message}"),
            Self::InvalidHashFormat(message) => write!(f, "Invalid hash format: {message}"),
            Self::BackgroundTaskFailed => write!(f, "Background task failed"),
            Self::ClaimSerialization(error) => {
                write!(f, "Claim serialization failed: {error}")
            }
        }
    }
}

impl Error for AuthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ClaimSerialization(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for AuthError {
    fn from(value: serde_json::Error) -> Self {
        Self::ClaimSerialization(value)
    }
}

pub fn generate_csrf_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    b64_urlsafe.encode(bytes)
}

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    Hasher::new()
        .algorithm(Algorithm::Argon2id)
        .hash_length(env::CONF.password_hash_length)
        .iterations(env::CONF.password_hash_iterations)
        .memory_cost_kib(env::CONF.password_hash_mem_cost_kib)
        .threads(env::CONF.password_hash_threads)
        .hash(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|e| AuthError::Hashing(format!("{e:?}")))
}

pub async fn hash_password_on_rayon(password: String) -> Result<String, AuthError> {
    let (tx, rx) = oneshot::channel();
    rayon::spawn(move || {
        let _ = tx.send(hash_password(&password));
    });
    rx.await.map_err(|_| AuthError::BackgroundTaskFailed)?
}

pub fn verify_password(password: &str, hash_str: &str) -> Result<bool, AuthError> {
    let hash = argon2_kdf::Hash::from_str(hash_str)
        .map_err(|e| AuthError::InvalidHashFormat(format!("{e:?}")))?;
    Ok(hash.verify(password.as_bytes()))
}

pub async fn verify_password_on_rayon(
    password: String,
    hash_str: String,
) -> Result<bool, AuthError> {
    let (tx, rx) = oneshot::channel();
    rayon::spawn(move || {
        let _ = tx.send(verify_password(&password, &hash_str));
    });
    rx.await.map_err(|_| AuthError::BackgroundTaskFailed)?
}

pub fn create_access_token(
    user_id: &str,
    token_version: i32,
    account_status: &str,
) -> Result<String, AuthError> {
    create_token(
        user_id,
        token_version,
        account_status,
        AuthTokenType::Access,
        env::CONF.access_token_lifetime.as_secs(),
    )
}

pub fn create_refresh_token(
    user_id: &str,
    token_version: i32,
    account_status: &str,
) -> Result<String, AuthError> {
    create_token(
        user_id,
        token_version,
        account_status,
        AuthTokenType::Refresh,
        env::CONF.refresh_token_lifetime.as_secs(),
    )
}

pub fn create_signin_token(
    user_id: &str,
    token_version: i32,
    account_status: &str,
) -> Result<String, AuthError> {
    create_token(
        user_id,
        token_version,
        account_status,
        AuthTokenType::SignIn,
        env::CONF.signin_token_lifetime.as_secs(),
    )
}

fn create_token(
    user_id: &str,
    token_version: i32,
    account_status: &str,
    token_type: AuthTokenType,
    lifetime_secs: u64,
) -> Result<String, AuthError> {
    let expiration = now_secs() + lifetime_secs;
    let claims = TokenClaims {
        user_id: user_id.to_string(),
        token_version,
        account_status: account_status.to_string(),
        expiration,
        token_type,
    };
    let mut payload = serde_json::to_vec(&claims)?;
    let sig = sign(&payload)?;
    payload.extend_from_slice(&sig);
    Ok(b64_urlsafe.encode(payload))
}

pub fn verify_access_token(token: &str) -> Result<TokenClaims, AuthError> {
    verify_token(token, AuthTokenType::Access)
}

pub fn verify_refresh_token(token: &str) -> Result<TokenClaims, AuthError> {
    verify_token(token, AuthTokenType::Refresh)
}

pub fn verify_signin_token(token: &str) -> Result<TokenClaims, AuthError> {
    verify_token(token, AuthTokenType::SignIn)
}

fn verify_token(token: &str, expected_type: AuthTokenType) -> Result<TokenClaims, AuthError> {
    const MAX_TOKEN_LENGTH: usize = 8192;
    const HMAC_SHA256_LEN: usize = 32;

    if token.len() > MAX_TOKEN_LENGTH {
        return Err(AuthError::TokenTooLong);
    }

    let decoded = b64_urlsafe
        .decode(token)
        .map_err(|_| AuthError::InvalidTokenEncoding)?;
    if decoded.len() <= HMAC_SHA256_LEN {
        return Err(AuthError::InvalidTokenFormat);
    }

    let json_len = decoded.len() - HMAC_SHA256_LEN;
    let json = &decoded[..json_len];
    let signature = &decoded[json_len..];

    let correct_signature = sign(json)?;
    constant_time_verify(&correct_signature, signature)?;

    let claims: TokenClaims = serde_json::from_slice(json).map_err(|_| AuthError::InvalidClaims)?;
    if claims.token_type != expected_type {
        return Err(AuthError::WrongTokenType);
    }
    if claims.expiration <= now_secs() {
        return Err(AuthError::TokenExpired);
    }

    Ok(claims)
}

fn sign(json: &[u8]) -> Result<Vec<u8>, AuthError> {
    let mut mac = HmacSha256::new_from_slice(env::CONF.cookie_signing_key.as_ref())
        .map_err(|_| AuthError::InvalidHmacKey)?;
    mac.update(json);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn constant_time_verify(a: &[u8], b: &[u8]) -> Result<(), AuthError> {
    if a.len() != b.len() || b.is_empty() {
        return Err(AuthError::InvalidSignature);
    }
    let mismatch = a
        .iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y));
    if mismatch == 0 {
        Ok(())
    } else {
        Err(AuthError::InvalidSignature)
    }
}

pub fn signature_hex_from_token(token: &str) -> Result<String, AuthError> {
    let decoded = b64_urlsafe
        .decode(token)
        .map_err(|_| AuthError::InvalidTokenEncoding)?;
    if decoded.len() < 32 {
        return Err(AuthError::InvalidTokenFormat);
    }
    let sig = &decoded[decoded.len() - 32..];
    Ok(sig.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn auth_cookie(name: &'static str, value: &str, max_age_secs: i64) -> Cookie<'static> {
    Cookie::build(name, value.to_string())
        .path("/")
        .http_only(true)
        .secure(env::CONF.secure_cookies)
        .same_site(SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(max_age_secs))
        .finish()
}

pub fn clear_auth_cookie(name: &'static str) -> Cookie<'static> {
    Cookie::build(name, "")
        .path("/")
        .http_only(true)
        .secure(env::CONF.secure_cookies)
        .same_site(SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish()
}

pub fn xsrf_cookie(token: &str, max_age_secs: i64) -> Cookie<'static> {
    Cookie::build("xsrf-token", token.to_string())
        .path("/")
        .http_only(false)
        .secure(env::CONF.secure_cookies)
        .same_site(SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(max_age_secs))
        .finish()
}

pub fn clear_xsrf_cookie() -> Cookie<'static> {
    Cookie::build("xsrf-token", "")
        .path("/")
        .http_only(false)
        .secure(env::CONF.secure_cookies)
        .same_site(SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish()
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_secs()
}
