use base64::Engine;
use base64::engine::general_purpose::STANDARD as b64;
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;
use zeroize::Zeroize;

pub static CONF: LazyLock<Config> = LazyLock::new(|| match Config::from_env() {
    Ok(conf) => conf,
    Err(error) => {
        if cfg!(test) {
            panic!("Failed to load config: {error}");
        } else {
            eprintln!("ERROR: Failed to load config: {error}");
            std::process::exit(1);
        }
    }
});

const DATABASE_URL_VAR: &str = "EQSHARING_DATABASE_URL";
const DB_MAX_CONNECTIONS_VAR: &str = "EQSHARING_DB_MAX_CONNECTIONS";
const COOKIE_SIGNING_KEY_VAR: &str = "EQSHARING_COOKIE_SIGNING_KEY_B64";
const PASSWORD_HASH_LENGTH_VAR: &str = "EQSHARING_PASSWORD_HASH_LENGTH";
const PASSWORD_HASH_ITERATIONS_VAR: &str = "EQSHARING_PASSWORD_HASH_ITERATIONS";
const PASSWORD_HASH_MEM_COST_KIB_VAR: &str = "EQSHARING_PASSWORD_HASH_MEM_COST_KIB";
const PASSWORD_HASH_THREADS_VAR: &str = "EQSHARING_PASSWORD_HASH_THREADS";
const ACCESS_TOKEN_LIFETIME_MINS_VAR: &str = "EQSHARING_ACCESS_TOKEN_LIFETIME_MINS";
const SIGNIN_TOKEN_LIFETIME_MINS_VAR: &str = "EQSHARING_SIGNIN_TOKEN_LIFETIME_MINS";
const REFRESH_TOKEN_LIFETIME_DAYS_VAR: &str = "EQSHARING_REFRESH_TOKEN_LIFETIME_DAYS";
const SECURE_COOKIES_VAR: &str = "EQSHARING_SECURE_COOKIES";
const BOOTSTRAP_ADMIN_EMAIL_VAR: &str = "EQSHARING_BOOTSTRAP_ADMIN_EMAIL";
const TEMP_PASSWORD_LENGTH_VAR: &str = "EQSHARING_TEMP_PASSWORD_LENGTH";
const RUN_BACKGROUND_TASKS_VAR: &str = "EQSHARING_RUN_BACKGROUND_TASKS";
const BLACKLIST_CLEANUP_JOB_FREQUENCY_SECS_VAR: &str =
    "EQSHARING_BLACKLIST_CLEANUP_JOB_FREQUENCY_SECS";
const BIND_HOST_VAR: &str = "EQSHARING_BIND_HOST";
const PORT_VAR: &str = "EQSHARING_PORT";
const ACTIX_WORKER_COUNT_VAR: &str = "EQSHARING_ACTIX_WORKER_COUNT";
const LOG_LEVEL_VAR: &str = "EQSHARING_LOG_LEVEL";
const LOG_PATH_VAR: &str = "EQSHARING_LOG_PATH";
const MAX_PASSWORD_LENGTH_VAR: &str = "EQSHARING_MAX_PASSWORD_LENGTH";
const TLS_CERT_PATH_VAR: &str = "EQSHARING_TLS_CERT_PATH";
const TLS_KEY_PATH_VAR: &str = "EQSHARING_TLS_KEY_PATH";
const PEER_IP_USE_X_FORWARDED_FOR_VAR: &str = "EQSHARING_PEER_IP_USE_X_FORWARDED_FOR";
const RATE_LIMITER_CLEAR_FREQUENCY_HOURS_VAR: &str = "EQSHARING_RATE_LIMITER_CLEAR_FREQUENCY_HOURS";
const RATE_LIMITER_WARN_EVERY_OVER_LIMIT_VAR: &str = "EQSHARING_RATE_LIMITER_WARN_EVERY_OVER_LIMIT";
const LIGHT_AUTH_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_LIGHT_AUTH_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const LIGHT_AUTH_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_LIGHT_AUTH_FAIR_USE_LIMITER_PERIOD_SECS";
const HEAVY_AUTH_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_HEAVY_AUTH_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const HEAVY_AUTH_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_HEAVY_AUTH_FAIR_USE_LIMITER_PERIOD_SECS";
const CREATE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_CREATE_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const CREATE_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_CREATE_FAIR_USE_LIMITER_PERIOD_SECS";
const POST_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_POST_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const POST_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str = "EQSHARING_POST_FAIR_USE_LIMITER_PERIOD_SECS";
const SURVEY_RESPONSE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_SURVEY_RESPONSE_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const SURVEY_RESPONSE_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_SURVEY_RESPONSE_FAIR_USE_LIMITER_PERIOD_SECS";
const READ_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_READ_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const READ_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str = "EQSHARING_READ_FAIR_USE_LIMITER_PERIOD_SECS";
const UPDATE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_UPDATE_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const UPDATE_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_UPDATE_FAIR_USE_LIMITER_PERIOD_SECS";
const DELETE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_DELETE_FAIR_USE_LIMITER_MAX_PER_PERIOD";
const DELETE_FAIR_USE_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_DELETE_FAIR_USE_LIMITER_PERIOD_SECS";
const LIGHT_AUTH_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_LIGHT_AUTH_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD";
const LIGHT_AUTH_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_LIGHT_AUTH_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS";
const HEAVY_AUTH_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_HEAVY_AUTH_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD";
const HEAVY_AUTH_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_HEAVY_AUTH_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS";
const CREATE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_CREATE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD";
const CREATE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_CREATE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS";
const READ_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_READ_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD";
const READ_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_READ_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS";
const UPDATE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_UPDATE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD";
const UPDATE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_UPDATE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS";
const DELETE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR: &str =
    "EQSHARING_DELETE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD";
const DELETE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR: &str =
    "EQSHARING_DELETE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS";
const CORS_ALLOWED_ORIGINS_VAR: &str = "EQSHARING_CORS_ALLOWED_ORIGINS";

const COOKIE_SIGNING_KEY_SIZE: usize = 64;

#[derive(Zeroize)]
pub struct ConfigInner {
    pub database_url: String,
    #[zeroize(skip)]
    pub db_max_connections: u32,
    pub cookie_signing_key: [u8; COOKIE_SIGNING_KEY_SIZE],
    #[zeroize(skip)]
    pub password_hash_length: u32,
    #[zeroize(skip)]
    pub password_hash_iterations: u32,
    #[zeroize(skip)]
    pub password_hash_mem_cost_kib: u32,
    #[zeroize(skip)]
    pub password_hash_threads: u32,
    #[zeroize(skip)]
    pub access_token_lifetime: Duration,
    #[zeroize(skip)]
    pub signin_token_lifetime: Duration,
    #[zeroize(skip)]
    pub refresh_token_lifetime: Duration,
    #[zeroize(skip)]
    pub secure_cookies: bool,
    #[zeroize(skip)]
    pub bootstrap_admin_email: String,
    #[zeroize(skip)]
    pub temp_password_length: usize,
    #[zeroize(skip)]
    pub run_background_tasks: bool,
    #[zeroize(skip)]
    pub blacklist_cleanup_job_frequency: Duration,
    #[zeroize(skip)]
    pub bind_host: String,
    #[zeroize(skip)]
    pub port: u16,
    #[zeroize(skip)]
    pub actix_worker_count: usize,
    #[zeroize(skip)]
    pub log_level: String,
    #[zeroize(skip)]
    pub log_path: Option<std::path::PathBuf>,
    #[zeroize(skip)]
    pub max_password_length: usize,
    #[zeroize(skip)]
    pub tls_cert_path: Option<std::path::PathBuf>,
    #[zeroize(skip)]
    pub tls_key_path: Option<std::path::PathBuf>,
    #[zeroize(skip)]
    pub peer_ip_use_x_forwarded_for: bool,
    #[zeroize(skip)]
    pub rate_limiter_clear_frequency: Duration,
    #[zeroize(skip)]
    pub rate_limiter_warn_every_over_limit: u32,
    #[zeroize(skip)]
    pub light_auth_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub light_auth_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub heavy_auth_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub heavy_auth_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub create_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub create_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub post_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub post_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub survey_response_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub survey_response_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub read_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub read_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub update_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub update_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub delete_fair_use_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub delete_fair_use_limiter_period: Duration,
    #[zeroize(skip)]
    pub light_auth_circuit_breaker_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub light_auth_circuit_breaker_limiter_period: Duration,
    #[zeroize(skip)]
    pub heavy_auth_circuit_breaker_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub heavy_auth_circuit_breaker_limiter_period: Duration,
    #[zeroize(skip)]
    pub create_circuit_breaker_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub create_circuit_breaker_limiter_period: Duration,
    #[zeroize(skip)]
    pub read_circuit_breaker_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub read_circuit_breaker_limiter_period: Duration,
    #[zeroize(skip)]
    pub update_circuit_breaker_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub update_circuit_breaker_limiter_period: Duration,
    #[zeroize(skip)]
    pub delete_circuit_breaker_limiter_max_per_period: u64,
    #[zeroize(skip)]
    pub delete_circuit_breaker_limiter_period: Duration,
    #[zeroize(skip)]
    pub cors_allowed_origins: Vec<String>,
}

pub struct Config {
    inner: UnsafeCell<ConfigInner>,
}

impl Deref for Config {
    type Target = ConfigInner;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.get() }
    }
}

unsafe impl Sync for Config {}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let cookie_signing_key = decode_fixed::<COOKIE_SIGNING_KEY_SIZE>(COOKIE_SIGNING_KEY_VAR)?;
        let cors_allowed_origins = env_var_or(
            CORS_ALLOWED_ORIGINS_VAR,
            String::from("http://localhost:5173,http://127.0.0.1:5173"),
        )?
        .split(',')
        .map(|origin| origin.trim().to_string())
        .filter(|origin| !origin.is_empty())
        .collect::<Vec<_>>();

        Ok(Self {
            inner: UnsafeCell::new(ConfigInner {
                database_url: env_var_or(DATABASE_URL_VAR, String::from("eq-sharing-test.db"))?,
                db_max_connections: env_var_or(DB_MAX_CONNECTIONS_VAR, 32)?,
                cookie_signing_key,
                password_hash_length: env_var_or(PASSWORD_HASH_LENGTH_VAR, 32)?,
                password_hash_iterations: env_var_or(PASSWORD_HASH_ITERATIONS_VAR, 4)?,
                password_hash_mem_cost_kib: env_var_or(PASSWORD_HASH_MEM_COST_KIB_VAR, 32768)?,
                password_hash_threads: env_var_or(PASSWORD_HASH_THREADS_VAR, 1)?,
                access_token_lifetime: Duration::from_secs(
                    env_var_or(ACCESS_TOKEN_LIFETIME_MINS_VAR, 15)? * 60,
                ),
                signin_token_lifetime: Duration::from_secs(
                    env_var_or(SIGNIN_TOKEN_LIFETIME_MINS_VAR, 10)? * 60,
                ),
                refresh_token_lifetime: Duration::from_secs(
                    env_var_or(REFRESH_TOKEN_LIFETIME_DAYS_VAR, 7)? * 86400,
                ),
                secure_cookies: env_var_or(SECURE_COOKIES_VAR, true)?,
                bootstrap_admin_email: env_var_or(
                    BOOTSTRAP_ADMIN_EMAIL_VAR,
                    String::from("admin@example.com"),
                )?
                .trim()
                .to_ascii_lowercase(),
                temp_password_length: env_var_or(TEMP_PASSWORD_LENGTH_VAR, 16)?,
                run_background_tasks: env_var_or(RUN_BACKGROUND_TASKS_VAR, true)?,
                blacklist_cleanup_job_frequency: Duration::from_secs(env_var_or(
                    BLACKLIST_CLEANUP_JOB_FREQUENCY_SECS_VAR,
                    86400,
                )?),
                bind_host: env_var_or(BIND_HOST_VAR, String::from("127.0.0.1"))?,
                port: env_var_or(PORT_VAR, 9000)?,
                actix_worker_count: env_var_or(
                    ACTIX_WORKER_COUNT_VAR,
                    std::thread::available_parallelism()
                        .map(std::num::NonZeroUsize::get)
                        .unwrap_or(1),
                )?,
                log_level: env_var_or(LOG_LEVEL_VAR, String::from("info"))?,
                log_path: std::env::var(LOG_PATH_VAR)
                    .ok()
                    .map(std::path::PathBuf::from),
                max_password_length: env_var_or(MAX_PASSWORD_LENGTH_VAR, 1024)?,
                tls_cert_path: std::env::var(TLS_CERT_PATH_VAR)
                    .ok()
                    .map(std::path::PathBuf::from),
                tls_key_path: std::env::var(TLS_KEY_PATH_VAR)
                    .ok()
                    .map(std::path::PathBuf::from),
                peer_ip_use_x_forwarded_for: env_var_or(PEER_IP_USE_X_FORWARDED_FOR_VAR, false)?,
                rate_limiter_clear_frequency: Duration::from_secs(
                    env_var_or(RATE_LIMITER_CLEAR_FREQUENCY_HOURS_VAR, 24)? * 3600,
                ),
                rate_limiter_warn_every_over_limit: env_var_or(
                    RATE_LIMITER_WARN_EVERY_OVER_LIMIT_VAR,
                    50,
                )?,
                light_auth_fair_use_limiter_max_per_period: env_var_or(
                    LIGHT_AUTH_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    16,
                )?,
                light_auth_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    LIGHT_AUTH_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    30,
                )?),
                heavy_auth_fair_use_limiter_max_per_period: env_var_or(
                    HEAVY_AUTH_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    8,
                )?,
                heavy_auth_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    HEAVY_AUTH_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    120,
                )?),
                create_fair_use_limiter_max_per_period: env_var_or(
                    CREATE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    20,
                )?,
                create_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    CREATE_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    30,
                )?),
                post_fair_use_limiter_max_per_period: env_var_or(
                    POST_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    2,
                )?,
                post_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    POST_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    600,
                )?),
                survey_response_fair_use_limiter_max_per_period: env_var_or(
                    SURVEY_RESPONSE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    1,
                )?,
                survey_response_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    SURVEY_RESPONSE_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    600,
                )?),
                read_fair_use_limiter_max_per_period: env_var_or(
                    READ_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    120,
                )?,
                read_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    READ_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    30,
                )?),
                update_fair_use_limiter_max_per_period: env_var_or(
                    UPDATE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    40,
                )?,
                update_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    UPDATE_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    30,
                )?),
                delete_fair_use_limiter_max_per_period: env_var_or(
                    DELETE_FAIR_USE_LIMITER_MAX_PER_PERIOD_VAR,
                    20,
                )?,
                delete_fair_use_limiter_period: Duration::from_secs(env_var_or(
                    DELETE_FAIR_USE_LIMITER_PERIOD_SECS_VAR,
                    30,
                )?),
                light_auth_circuit_breaker_limiter_max_per_period: env_var_or(
                    LIGHT_AUTH_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR,
                    2000,
                )?,
                light_auth_circuit_breaker_limiter_period: Duration::from_secs(env_var_or(
                    LIGHT_AUTH_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR,
                    60,
                )?),
                heavy_auth_circuit_breaker_limiter_max_per_period: env_var_or(
                    HEAVY_AUTH_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR,
                    200,
                )?,
                heavy_auth_circuit_breaker_limiter_period: Duration::from_secs(env_var_or(
                    HEAVY_AUTH_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR,
                    60,
                )?),
                create_circuit_breaker_limiter_max_per_period: env_var_or(
                    CREATE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR,
                    3000,
                )?,
                create_circuit_breaker_limiter_period: Duration::from_secs(env_var_or(
                    CREATE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR,
                    60,
                )?),
                read_circuit_breaker_limiter_max_per_period: env_var_or(
                    READ_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR,
                    5000,
                )?,
                read_circuit_breaker_limiter_period: Duration::from_secs(env_var_or(
                    READ_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR,
                    60,
                )?),
                update_circuit_breaker_limiter_max_per_period: env_var_or(
                    UPDATE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR,
                    3500,
                )?,
                update_circuit_breaker_limiter_period: Duration::from_secs(env_var_or(
                    UPDATE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR,
                    60,
                )?),
                delete_circuit_breaker_limiter_max_per_period: env_var_or(
                    DELETE_CIRCUIT_BREAKER_LIMITER_MAX_PER_PERIOD_VAR,
                    2500,
                )?,
                delete_circuit_breaker_limiter_period: Duration::from_secs(env_var_or(
                    DELETE_CIRCUIT_BREAKER_LIMITER_PERIOD_SECS_VAR,
                    60,
                )?),
                cors_allowed_origins,
            }),
        })
    }
}

#[cfg(test)]
pub fn set_peer_ip_use_x_forwarded_for_for_test(value: bool) {
    unsafe {
        (*CONF.inner.get()).peer_ip_use_x_forwarded_for = value;
    }
}

fn decode_fixed<const N: usize>(name: &'static str) -> Result<[u8; N], ConfigError> {
    let raw = match std::env::var(name) {
        Ok(value) => value,
        Err(_) if cfg!(test) => b64.encode(vec![0u8; N]),
        Err(_) => return Err(ConfigError::MissingVar(name)),
    };
    let decoded = b64
        .decode(raw.as_bytes())
        .map_err(|_| ConfigError::InvalidVar(name))?;
    decoded[..N]
        .try_into()
        .map_err(|_| ConfigError::InvalidVar(name))
}

#[allow(dead_code)]
fn env_var<T: FromStr>(name: &'static str) -> Result<T, ConfigError> {
    let value = std::env::var(name).map_err(|_| ConfigError::MissingVar(name))?;
    value
        .parse::<T>()
        .map_err(|_| ConfigError::InvalidVar(name))
}

fn env_var_or<T: FromStr>(name: &'static str, default: T) -> Result<T, ConfigError> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<T>()
            .map_err(|_| ConfigError::InvalidVar(name)),
        Err(_) => Ok(default),
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingVar(&'static str),
    InvalidVar(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVar(name) => write!(f, "Missing environment variable: {name}"),
            Self::InvalidVar(name) => write!(f, "Invalid environment variable: {name}"),
        }
    }
}

impl Error for ConfigError {}
