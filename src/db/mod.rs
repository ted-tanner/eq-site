use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Error as PoolError, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::error::Error;
use std::fmt;

pub mod admin;
pub mod auth;
pub mod feed;
pub mod notification;
pub mod public;
pub mod user;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[derive(Debug, Default)]
struct SqliteConnectionCustomizer;

impl CustomizeConnection<SqliteConnection, PoolError> for SqliteConnectionCustomizer {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), PoolError> {
        configure_sqlite_connection(conn).map_err(Into::into)
    }
}

pub fn create_db_pool(database_url: &str, max_connections: u32) -> DbPool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .max_size(max_connections)
        .connection_customizer(Box::new(SqliteConnectionCustomizer))
        .build(manager)
        .expect("create sqlite pool")
}

pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), DaoError> {
    configure_sqlite_connection(conn).map_err(DaoError::from)?;
    conn.run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(|e| DaoError::StateError(e.to_string()))
}

fn configure_sqlite_connection(conn: &mut SqliteConnection) -> QueryResult<()> {
    conn.batch_execute(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA wal_autocheckpoint = 1000;
        PRAGMA temp_store = MEMORY;
        ",
    )?;
    Ok(())
}

pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time after epoch")
        .as_secs() as i64
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum DaoError {
    NotFound,
    InvalidInput(String),
    Requirement(String),
    PoolFailure(String),
    StateError(String),
    QueryFailure(diesel::result::Error),
    ConnectionFailure(diesel::ConnectionError),
}

impl fmt::Display for DaoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Not found"),
            Self::InvalidInput(message) => write!(f, "Invalid input: {message}"),
            Self::Requirement(message) => write!(f, "Requirement failed: {message}"),
            Self::PoolFailure(message) => write!(f, "Pool failure: {message}"),
            Self::StateError(message) => write!(f, "State error: {message}"),
            Self::QueryFailure(error) => write!(f, "{error}"),
            Self::ConnectionFailure(error) => write!(f, "{error}"),
        }
    }
}

impl Error for DaoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::QueryFailure(error) => Some(error),
            Self::ConnectionFailure(error) => Some(error),
            _ => None,
        }
    }
}

impl From<diesel::result::Error> for DaoError {
    fn from(value: diesel::result::Error) -> Self {
        Self::QueryFailure(value)
    }
}

impl From<diesel::ConnectionError> for DaoError {
    fn from(value: diesel::ConnectionError) -> Self {
        Self::ConnectionFailure(value)
    }
}

impl From<r2d2::Error> for DaoError {
    fn from(value: r2d2::Error) -> Self {
        DaoError::PoolFailure(value.to_string())
    }
}
