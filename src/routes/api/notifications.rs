use actix_web::web::*;

use crate::handlers::notification_handler;
use crate::middleware;

use super::RateLimiters;

pub fn configure(cfg: &mut ServiceConfig, limiters: RateLimiters) {
    cfg.service(
        scope("/notifications")
            .service(
                resource("")
                    .route(get().to(notification_handler::list_notifications))
                    .wrap(limiters.read_fair_use.clone())
                    .wrap(limiters.read_circuit_breaker.clone()),
            )
            .service(
                resource("/read")
                    .route(post().to(notification_handler::mark_notifications_read))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.update_fair_use.clone())
                    .wrap(limiters.update_circuit_breaker.clone()),
            )
            .service(
                resource("/clear")
                    .route(delete().to(notification_handler::clear_notifications))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.update_fair_use.clone())
                    .wrap(limiters.update_circuit_breaker.clone()),
            ),
    );
}
