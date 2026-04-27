use actix_web::dev::Payload;
use actix_web::{Error, FromRequest, HttpRequest, error::ErrorUnauthorized};
use futures::future::LocalBoxFuture;

use crate::AppState;
use crate::services::auth::AuthService;

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub account_status: String,
}

#[derive(Clone, Debug)]
pub struct SignInOnlyUser {
    pub user_id: String,
}

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let state = req.app_data::<actix_web::web::Data<AppState>>().cloned();
        let token = req
            .cookie("access_token")
            .map(|cookie| cookie.value().to_string());
        Box::pin(async move {
            let state = state.ok_or_else(|| ErrorUnauthorized("Missing app state"))?;
            let token = token.ok_or_else(|| ErrorUnauthorized("Missing access token"))?;
            let session = AuthService::new(&state.db_pool)
                .validate_access_token(token)
                .await
                .map_err(|_| ErrorUnauthorized("Invalid access token"))?;
            Ok(Self {
                user_id: session.user_id,
                account_status: session.account_status,
            })
        })
    }
}

impl FromRequest for SignInOnlyUser {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let state = req.app_data::<actix_web::web::Data<AppState>>().cloned();
        let token = req
            .cookie("sign_in_token")
            .map(|cookie| cookie.value().to_string());
        Box::pin(async move {
            let state = state.ok_or_else(|| ErrorUnauthorized("Missing app state"))?;
            let token = token.ok_or_else(|| ErrorUnauthorized("Missing sign-in token"))?;
            let user_id = AuthService::new(&state.db_pool)
                .validate_signin_token(token)
                .await
                .map_err(|_| ErrorUnauthorized("Invalid sign-in token"))?;
            Ok(Self { user_id })
        })
    }
}
