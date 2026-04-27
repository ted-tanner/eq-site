use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use std::borrow::Cow;
use std::fmt;

pub mod admin_handler;
pub mod auth_handler;
pub mod feed_handler;
pub mod notification_handler;
pub mod public_handler;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug)]
pub enum HandlerError {
    BadRequest(Cow<'static, str>),
    Unauthorized(Cow<'static, str>),
    Forbidden(Cow<'static, str>),
    NotFound(Cow<'static, str>),
    Conflict(Cow<'static, str>),
    Internal(Cow<'static, str>),
}

impl HandlerError {
    pub fn bad_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::BadRequest(message.into())
    }
    pub fn unauthorized(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Unauthorized(message.into())
    }
    pub fn not_found(message: impl Into<Cow<'static, str>>) -> Self {
        Self::NotFound(message.into())
    }
    pub fn internal(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Internal(message.into())
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            HandlerError::BadRequest(msg)
            | HandlerError::Unauthorized(msg)
            | HandlerError::Forbidden(msg)
            | HandlerError::NotFound(msg)
            | HandlerError::Conflict(msg)
            | HandlerError::Internal(msg) => msg,
        };
        write!(f, "{message}")
    }
}

impl ResponseError for HandlerError {
    fn status_code(&self) -> StatusCode {
        match self {
            HandlerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            HandlerError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            HandlerError::Forbidden(_) => StatusCode::FORBIDDEN,
            HandlerError::NotFound(_) => StatusCode::NOT_FOUND,
            HandlerError::Conflict(_) => StatusCode::CONFLICT,
            HandlerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error: self.to_string(),
        })
    }
}

impl From<crate::db::DaoError> for HandlerError {
    fn from(value: crate::db::DaoError) -> Self {
        match value {
            crate::db::DaoError::NotFound => HandlerError::not_found("Not found"),
            crate::db::DaoError::InvalidInput(message)
            | crate::db::DaoError::Requirement(message) => HandlerError::bad_request(message),
            crate::db::DaoError::PoolFailure(message)
            | crate::db::DaoError::StateError(message) => HandlerError::internal(message),
            crate::db::DaoError::QueryFailure(diesel::result::Error::DatabaseError(_, _)) => {
                HandlerError::internal("Database error")
            }
            crate::db::DaoError::QueryFailure(diesel::result::Error::NotFound) => {
                HandlerError::not_found("Not found")
            }
            crate::db::DaoError::QueryFailure(error) => HandlerError::internal(error.to_string()),
            crate::db::DaoError::ConnectionFailure(error) => {
                HandlerError::internal(error.to_string())
            }
        }
    }
}

impl From<crate::services::ServiceError> for HandlerError {
    fn from(value: crate::services::ServiceError) -> Self {
        match value {
            crate::services::ServiceError::BadRequest(message) => HandlerError::BadRequest(message),
            crate::services::ServiceError::Unauthorized(message) => {
                HandlerError::Unauthorized(message)
            }
            crate::services::ServiceError::Forbidden(message) => HandlerError::Forbidden(message),
            crate::services::ServiceError::NotFound(message) => HandlerError::NotFound(message),
            crate::services::ServiceError::Conflict(message) => HandlerError::Conflict(message),
            crate::services::ServiceError::Internal(message) => HandlerError::Internal(message),
        }
    }
}
