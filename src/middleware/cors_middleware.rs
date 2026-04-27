use std::future::{Ready, ready};

use actix_web::{
    Error, HttpResponse,
    body::{BoxBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{self, HeaderValue},
};
use futures::future::LocalBoxFuture;

pub struct CorsMiddleware {
    allowed_origins: Vec<String>,
}

impl Default for CorsMiddleware {
    fn default() -> Self {
        Self {
            allowed_origins: crate::env::CONF.cors_allowed_origins.clone(),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for CorsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = CorsMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let allowed_origin_headers = self
            .allowed_origins
            .iter()
            .filter_map(|origin| {
                HeaderValue::from_str(origin)
                    .ok()
                    .map(|hv| (origin.clone(), hv))
            })
            .collect::<Vec<_>>();
        ready(Ok(CorsMiddlewareService {
            service,
            allowed_origin_headers,
        }))
    }
}

pub struct CorsMiddlewareService<S> {
    service: S,
    allowed_origin_headers: Vec<(String, HeaderValue)>,
}

impl<S, B> Service<ServiceRequest> for CorsMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let is_options = req.method() == actix_web::http::Method::OPTIONS;
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|h| h.to_str().ok())
            .map(str::to_string);

        let allowed_origin = origin.as_ref().and_then(|o| {
            self.allowed_origin_headers
                .iter()
                .find(|(allowed, _)| allowed == o)
                .map(|(_, hv)| hv.clone())
        });

        if is_options {
            let (parts, _) = req.into_parts();
            let mut res = HttpResponse::Ok();
            if let Some(origin_header) = allowed_origin {
                res.insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, origin_header));
                res.insert_header((
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
                ));
                res.insert_header((
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_static("content-type, x-xsrf-token"),
                ));
                res.insert_header((
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                ));
                res.insert_header((
                    header::ACCESS_CONTROL_MAX_AGE,
                    HeaderValue::from_static("86400"),
                ));
            }
            let res = ServiceResponse::new(parts, res.finish()).map_into_boxed_body();
            return Box::pin(async move { Ok(res) });
        }

        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?.map_into_boxed_body();
            if let Some(origin_header) = allowed_origin {
                res.headers_mut()
                    .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin_header);
                res.headers_mut().insert(
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                );
            }
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpResponse, http::StatusCode, http::header, test, web};

    fn test_middleware() -> CorsMiddleware {
        CorsMiddleware {
            allowed_origins: vec!["https://app.example.com".to_string()],
        }
    }

    async fn ok_handler() -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    #[actix_web::test]
    async fn options_preflight_for_allowed_origin_sets_cors_headers() {
        let app = test::init_service(
            App::new()
                .wrap(test_middleware())
                .route("/", web::get().to(ok_handler)),
        )
        .await;

        let req = test::TestRequest::default()
            .method(actix_web::http::Method::OPTIONS)
            .uri("/")
            .insert_header((header::ORIGIN, "https://app.example.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("https://app.example.com")
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .and_then(|v| v.to_str().ok()),
            Some("GET, POST, PUT, PATCH, DELETE, OPTIONS")
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .and_then(|v| v.to_str().ok()),
            Some("content-type, x-xsrf-token")
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .and_then(|v| v.to_str().ok()),
            Some("true")
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_MAX_AGE)
                .and_then(|v| v.to_str().ok()),
            Some("86400")
        );
    }

    #[actix_web::test]
    async fn options_preflight_for_disallowed_origin_omits_cors_headers() {
        let app = test::init_service(
            App::new()
                .wrap(test_middleware())
                .route("/", web::get().to(ok_handler)),
        )
        .await;

        let req = test::TestRequest::default()
            .method(actix_web::http::Method::OPTIONS)
            .uri("/")
            .insert_header((header::ORIGIN, "https://evil.example.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .is_none()
        );
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .is_none()
        );
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
        assert!(resp.headers().get(header::ACCESS_CONTROL_MAX_AGE).is_none());
    }

    #[actix_web::test]
    async fn normal_request_for_allowed_origin_sets_response_headers() {
        let app = test::init_service(
            App::new()
                .wrap(test_middleware())
                .route("/", web::get().to(ok_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header((header::ORIGIN, "https://app.example.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("https://app.example.com")
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .and_then(|v| v.to_str().ok()),
            Some("true")
        );
    }

    #[actix_web::test]
    async fn normal_request_for_disallowed_origin_has_no_cors_headers() {
        let app = test::init_service(
            App::new()
                .wrap(test_middleware())
                .route("/", web::get().to(ok_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header((header::ORIGIN, "https://evil.example.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
    }
}
