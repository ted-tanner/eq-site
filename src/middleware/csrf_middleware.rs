//! CSRF middleware: double-submit cookie. For POST/PUT/PATCH/DELETE, require
//! x-xsrf-token header to match xsrf-token cookie (constant-time).

use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::Method,
};
use futures::future::LocalBoxFuture;
use std::future::{Ready, ready};

/// Constant-time comparison for CSRF token.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub struct CsrfMiddleware;

impl<S, B> Transform<S, ServiceRequest> for CsrfMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = CsrfMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CsrfMiddlewareService { service }))
    }
}

pub struct CsrfMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CsrfMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let method = req.method().clone();
        let path = req.path().to_string();
        let cookie_value = req.cookie("xsrf-token").map(|c| c.value().to_string());
        let header_value = req
            .headers()
            .get("x-xsrf-token")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let exempt_path = path == "/api/auth/sign-in" || path == "/api/auth/create-user";

        let allowed = match method {
            Method::GET | Method::HEAD | Method::OPTIONS => true,
            Method::POST | Method::PUT | Method::PATCH | Method::DELETE => {
                if exempt_path {
                    true
                } else {
                    matches!(
                        (cookie_value.as_deref(), header_value.as_deref()),
                        (Some(c), Some(h)) if constant_time_eq(c, h)
                    )
                }
            }
            _ => true,
        };

        if !allowed {
            return Box::pin(async move {
                Err(actix_web::error::ErrorForbidden(
                    "Invalid or missing CSRF token",
                ))
            });
        }

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};

    async fn ok_handler() -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    async fn post_handler() -> HttpResponse {
        HttpResponse::Ok().body("ok")
    }

    #[actix_web::test]
    async fn get_requests_pass_without_csrf() {
        let app = test::init_service(
            App::new()
                .wrap(CsrfMiddleware)
                .route("/", web::get().to(ok_handler))
                .route("/", web::post().to(post_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn post_without_cookie_rejected() {
        let app = test::init_service(
            App::new()
                .wrap(CsrfMiddleware)
                .route("/", web::post().to(post_handler)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/")
            .set_payload("{}")
            .to_request();
        let result = test::try_call_service(&app, req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CSRF"));
    }

    #[actix_web::test]
    async fn post_with_mismatch_rejected() {
        let app = test::init_service(
            App::new()
                .wrap(CsrfMiddleware)
                .route("/", web::post().to(post_handler)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/")
            .cookie(actix_web::cookie::Cookie::new("xsrf-token", "abc"))
            .insert_header(("x-xsrf-token", "xyz"))
            .set_payload("{}")
            .to_request();
        let result = test::try_call_service(&app, req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CSRF"));
    }

    #[actix_web::test]
    async fn post_with_match_accepted() {
        let app = test::init_service(
            App::new()
                .wrap(CsrfMiddleware)
                .route("/", web::post().to(post_handler)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/")
            .cookie(actix_web::cookie::Cookie::new("xsrf-token", "same-token"))
            .insert_header(("x-xsrf-token", "same-token"))
            .set_payload("{}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
