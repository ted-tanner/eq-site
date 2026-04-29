use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth;
use crate::handlers::HandlerError;
use crate::middleware::auth_middleware::{AuthenticatedUser, SignInOnlyUser};
use crate::models::user::User;
use crate::services::auth::{AuthService, ChangePasswordServiceInput, LoginSession, SignUpInput};

#[derive(Debug, Deserialize)]
pub struct SignUpRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SignInRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteOwnAccountRequest {
    pub current_password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_kind: String,
    pub user: serde_json::Value,
}

fn user_json(user: &User) -> serde_json::Value {
    serde_json::json!({
        "id": user.id,
        "email": user.email,
        "first_name": user.first_name,
        "last_name": user.last_name,
        "is_admin": user.is_admin,
        "account_status": user.account_status,
        "must_change_password": user.must_change_password
    })
}

fn session_response(user: &User, session_kind: &str) -> HttpResponse {
    HttpResponse::Ok().json(SessionResponse {
        session_kind: session_kind.to_string(),
        user: user_json(user),
    })
}

fn login_session_response(session: LoginSession) -> Result<HttpResponse, HandlerError> {
    match session {
        LoginSession::Full { user, tokens } => {
            let session_kind = if user.account_status == crate::models::user::ACCOUNT_PENDING {
                "signed_up"
            } else {
                "full"
            };
            Ok(HttpResponse::Ok()
                .cookie(auth::auth_cookie(
                    "access_token",
                    &tokens.access_token,
                    crate::env::CONF.access_token_lifetime.as_secs() as i64,
                ))
                .cookie(auth::auth_cookie(
                    "refresh_token",
                    &tokens.refresh_token,
                    crate::env::CONF.refresh_token_lifetime.as_secs() as i64,
                ))
                .cookie(auth::xsrf_cookie(
                    &tokens.csrf_token,
                    crate::env::CONF.access_token_lifetime.as_secs() as i64,
                ))
                .cookie(auth::clear_auth_cookie("sign_in_token"))
                .json(SessionResponse {
                    session_kind: session_kind.to_string(),
                    user: user_json(&user),
                }))
        }
        LoginSession::PasswordChangeRequired { user, token } => Ok(HttpResponse::Ok()
            .cookie(auth::auth_cookie(
                "sign_in_token",
                &token.sign_in_token,
                crate::env::CONF.signin_token_lifetime.as_secs() as i64,
            ))
            .cookie(auth::clear_auth_cookie("access_token"))
            .cookie(auth::clear_auth_cookie("refresh_token"))
            .cookie(auth::xsrf_cookie(
                &token.csrf_token,
                crate::env::CONF.signin_token_lifetime.as_secs() as i64,
            ))
            .json(SessionResponse {
                session_kind: "password_change_required".to_string(),
                user: user_json(&user),
            })),
    }
}

pub async fn get_csrf_token() -> HttpResponse {
    let token = auth::generate_csrf_token();
    HttpResponse::Ok()
        .cookie(auth::xsrf_cookie(
            &token,
            crate::env::CONF.access_token_lifetime.as_secs() as i64,
        ))
        .finish()
}

pub async fn sign_up(
    state: web::Data<AppState>,
    body: web::Json<SignUpRequest>,
) -> Result<HttpResponse, HandlerError> {
    let (user, tokens) = AuthService::new(&state.db_pool)
        .sign_up(SignUpInput {
            first_name: body.first_name.clone(),
            last_name: body.last_name.clone(),
            email: body.email.clone(),
            password: body.password.clone(),
        })
        .await?;
    Ok(HttpResponse::Ok()
        .cookie(auth::auth_cookie(
            "access_token",
            &tokens.access_token,
            crate::env::CONF.access_token_lifetime.as_secs() as i64,
        ))
        .cookie(auth::auth_cookie(
            "refresh_token",
            &tokens.refresh_token,
            crate::env::CONF.refresh_token_lifetime.as_secs() as i64,
        ))
        .cookie(auth::xsrf_cookie(
            &tokens.csrf_token,
            crate::env::CONF.access_token_lifetime.as_secs() as i64,
        ))
        .cookie(auth::clear_auth_cookie("sign_in_token"))
        .json(SessionResponse {
            session_kind: "signed_up".to_string(),
            user: user_json(&user),
        }))
}

pub async fn sign_in(
    state: web::Data<AppState>,
    body: web::Json<SignInRequest>,
) -> Result<HttpResponse, HandlerError> {
    let session = AuthService::new(&state.db_pool)
        .sign_in(body.email.clone(), body.password.clone())
        .await?;
    login_session_response(session)
}

pub async fn session(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, HandlerError> {
    let access_token = req
        .cookie("access_token")
        .map(|cookie| cookie.value().to_string());
    let sign_in_token = req
        .cookie("sign_in_token")
        .map(|cookie| cookie.value().to_string());
    let (user, session_kind) = AuthService::new(&state.db_pool)
        .session_from_tokens(access_token, sign_in_token)
        .await?;
    Ok(session_response(&user, session_kind))
}

pub async fn refresh_tokens(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, HandlerError> {
    let refresh_token = req
        .cookie("refresh_token")
        .ok_or_else(|| HandlerError::unauthorized("Missing refresh token"))?
        .value()
        .to_string();
    let session = AuthService::new(&state.db_pool)
        .refresh_tokens(refresh_token)
        .await?;
    login_session_response(session)
}

pub async fn logout(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, HandlerError> {
    let refresh_token = req
        .cookie("refresh_token")
        .map(|cookie| cookie.value().to_string());
    AuthService::new(&state.db_pool)
        .logout(refresh_token)
        .await?;
    Ok(HttpResponse::Ok()
        .cookie(auth::clear_auth_cookie("access_token"))
        .cookie(auth::clear_auth_cookie("refresh_token"))
        .cookie(auth::clear_auth_cookie("sign_in_token"))
        .cookie(auth::clear_xsrf_cookie())
        .finish())
}

pub async fn change_password(
    state: web::Data<AppState>,
    auth_user: Option<AuthenticatedUser>,
    sign_in_user: Option<SignInOnlyUser>,
    body: web::Json<ChangePasswordRequest>,
) -> Result<HttpResponse, HandlerError> {
    let user_id = auth_user
        .map(|u| u.user_id)
        .or_else(|| sign_in_user.map(|u| u.user_id))
        .ok_or_else(|| HandlerError::unauthorized("Missing authentication"))?;
    let session = AuthService::new(&state.db_pool)
        .change_password(ChangePasswordServiceInput {
            user_id,
            current_password: body.current_password.clone(),
            new_password: body.new_password.clone(),
        })
        .await?;
    login_session_response(session)
}

pub async fn delete_own_account(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    req: HttpRequest,
    body: web::Json<DeleteOwnAccountRequest>,
) -> Result<HttpResponse, HandlerError> {
    let refresh_token = req
        .cookie("refresh_token")
        .map(|cookie| cookie.value().to_string());
    AuthService::new(&state.db_pool)
        .delete_own_account(
            auth_user.user_id,
            body.current_password.clone(),
            refresh_token,
        )
        .await?;
    Ok(HttpResponse::Ok()
        .cookie(auth::clear_auth_cookie("access_token"))
        .cookie(auth::clear_auth_cookie("refresh_token"))
        .cookie(auth::clear_auth_cookie("sign_in_token"))
        .cookie(auth::clear_xsrf_cookie())
        .finish())
}
