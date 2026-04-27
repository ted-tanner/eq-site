use actix_web::{HttpResponse, ResponseError, http::StatusCode, web};
use serde::Serialize;
use std::borrow::Cow;
use std::fmt;

use crate::db::DaoError;

pub mod admin;
pub mod auth;
pub mod feed;
pub mod notification;
pub mod public;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug)]
pub enum ServiceError {
    BadRequest(Cow<'static, str>),
    Unauthorized(Cow<'static, str>),
    Forbidden(Cow<'static, str>),
    NotFound(Cow<'static, str>),
    Conflict(Cow<'static, str>),
    Internal(Cow<'static, str>),
}

impl ServiceError {
    pub fn bad_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn unauthorized(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn forbidden(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn not_found(message: impl Into<Cow<'static, str>>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn internal(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Internal(message.into())
    }

    fn from_dao_error(value: DaoError) -> Self {
        match value {
            DaoError::NotFound => ServiceError::not_found("Not found"),
            DaoError::InvalidInput(message) | DaoError::Requirement(message) => {
                ServiceError::bad_request(message)
            }
            DaoError::PoolFailure(message) => {
                log::error!("DAO pool failure: {message}");
                ServiceError::internal(message)
            }
            DaoError::StateError(message) => {
                log::error!("DAO state error: {message}");
                ServiceError::internal(message)
            }
            DaoError::QueryFailure(diesel::result::Error::NotFound) => {
                ServiceError::not_found("Not found")
            }
            DaoError::QueryFailure(error @ diesel::result::Error::DatabaseError(_, _)) => {
                log::error!("database query failed: {error}");
                ServiceError::internal("Database error")
            }
            DaoError::QueryFailure(error) => {
                log::error!("database query failed: {error}");
                ServiceError::internal(error.to_string())
            }
            DaoError::ConnectionFailure(error) => {
                log::error!("database connection failed: {error}");
                ServiceError::internal(error.to_string())
            }
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ServiceError::BadRequest(msg)
            | ServiceError::Unauthorized(msg)
            | ServiceError::Forbidden(msg)
            | ServiceError::NotFound(msg)
            | ServiceError::Conflict(msg)
            | ServiceError::Internal(msg) => msg,
        };
        write!(f, "{message}")
    }
}

impl ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        match self {
            ServiceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServiceError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ServiceError::Forbidden(_) => StatusCode::FORBIDDEN,
            ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            ServiceError::Conflict(_) => StatusCode::CONFLICT,
            ServiceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error: self.to_string(),
        })
    }
}

impl From<DaoError> for ServiceError {
    fn from(value: DaoError) -> Self {
        ServiceError::from_dao_error(value)
    }
}

impl From<crate::auth::AuthError> for ServiceError {
    fn from(value: crate::auth::AuthError) -> Self {
        log::error!("auth operation failed: {value}");
        ServiceError::internal(value.to_string())
    }
}

pub async fn block_dao<T, F>(work: F) -> Result<T, ServiceError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, DaoError> + Send + 'static,
{
    match web::block(work).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(ServiceError::from_dao_error(error)),
        Err(error) => {
            log::error!("blocking DAO task failed: {error}");
            Err(ServiceError::internal("Database task failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::db::DaoError;

    use super::{ServiceError, block_dao};

    #[actix_rt::test]
    async fn block_dao_preserves_explicit_not_found() {
        let error = block_dao::<(), _>(|| Err(DaoError::NotFound))
            .await
            .expect_err("expected error");
        assert!(matches!(error, ServiceError::NotFound(_)));
    }

    #[actix_rt::test]
    async fn block_dao_preserves_diesel_not_found() {
        let error =
            block_dao::<(), _>(|| Err(DaoError::QueryFailure(diesel::result::Error::NotFound)))
                .await
                .expect_err("expected error");
        assert!(matches!(error, ServiceError::NotFound(_)));
    }

    #[actix_rt::test]
    async fn block_dao_maps_unexpected_dao_errors_to_internal() {
        let error = block_dao::<(), _>(|| Err(DaoError::StateError("broken".to_string())))
            .await
            .expect_err("expected error");
        assert!(matches!(error, ServiceError::Internal(_)));
    }
}
