use actix_web::web::*;

use crate::handlers::auth_handler;
use crate::middleware;

use super::RateLimiters;

pub fn configure(cfg: &mut ServiceConfig, limiters: RateLimiters) {
    cfg.service(
        scope("/auth")
            .service(
                resource("/csrf-token")
                    .route(get().to(auth_handler::get_csrf_token))
                    .wrap(limiters.read_fair_use.clone())
                    .wrap(limiters.read_circuit_breaker.clone()),
            )
            .service(
                resource("/sign-up")
                    .route(post().to(auth_handler::sign_up))
                    .wrap(limiters.create_fair_use.clone())
                    .wrap(limiters.create_circuit_breaker.clone()),
            )
            .service(
                resource("/sign-in")
                    .route(post().to(auth_handler::sign_in))
                    .wrap(limiters.heavy_auth_fair_use.clone())
                    .wrap(limiters.heavy_auth_circuit_breaker.clone()),
            )
            .service(
                resource("/session")
                    .route(get().to(auth_handler::session))
                    .wrap(limiters.read_fair_use.clone())
                    .wrap(limiters.read_circuit_breaker.clone()),
            )
            .service(
                resource("/refresh")
                    .route(post().to(auth_handler::refresh_tokens))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.light_auth_fair_use.clone())
                    .wrap(limiters.light_auth_circuit_breaker.clone()),
            )
            .service(
                resource("/logout")
                    .route(post().to(auth_handler::logout))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.light_auth_fair_use.clone())
                    .wrap(limiters.light_auth_circuit_breaker.clone()),
            )
            .service(
                resource("/change-password")
                    .route(post().to(auth_handler::change_password))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.heavy_auth_fair_use.clone())
                    .wrap(limiters.heavy_auth_circuit_breaker.clone()),
            )
            .service(
                resource("/delete-account")
                    .route(delete().to(auth_handler::delete_own_account))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.delete_fair_use.clone())
                    .wrap(limiters.delete_circuit_breaker.clone()),
            ),
    );
}
