use actix_web::web::*;

use crate::handlers::public_handler;

use super::RateLimiters;

pub fn configure(cfg: &mut ServiceConfig, limiters: RateLimiters) {
    cfg.service(
        scope("/public")
            .service(
                resource("/landing")
                    .route(get().to(public_handler::landing))
                    .wrap(limiters.read_fair_use.clone())
                    .wrap(limiters.read_circuit_breaker.clone()),
            )
            .service(
                resource("/study-topics/upcoming")
                    .route(get().to(public_handler::upcoming_study_topics))
                    .wrap(limiters.read_fair_use.clone())
                    .wrap(limiters.read_circuit_breaker.clone()),
            ),
    );
}
