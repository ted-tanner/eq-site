use actix_web::web::*;

use crate::handlers::feed_handler;
use crate::middleware;

use super::RateLimiters;

pub fn configure(cfg: &mut ServiceConfig, limiters: RateLimiters) {
    cfg.service(
        scope("/feed")
            .service(
                resource("/posts")
                    .route(
                        get()
                            .to(feed_handler::list_posts)
                            .wrap(limiters.read_fair_use.clone())
                            .wrap(limiters.read_circuit_breaker.clone()),
                    )
                    .route(
                        post()
                            .to(feed_handler::create_post)
                            .wrap(middleware::csrf_middleware::CsrfMiddleware)
                            .wrap(limiters.post_fair_use.clone())
                            .wrap(limiters.create_circuit_breaker.clone()),
                    ),
            )
            .service(
                resource("/posts/{post_id}")
                    .route(
                        get()
                            .to(feed_handler::get_post_thread)
                            .wrap(limiters.read_fair_use.clone())
                            .wrap(limiters.read_circuit_breaker.clone()),
                    )
                    .route(
                        delete()
                            .to(feed_handler::delete_post)
                            .wrap(middleware::csrf_middleware::CsrfMiddleware)
                            .wrap(limiters.delete_fair_use.clone())
                            .wrap(limiters.delete_circuit_breaker.clone()),
                    ),
            )
            .service(
                resource("/posts/{post_id}/replies")
                    .route(post().to(feed_handler::create_reply))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.create_fair_use.clone())
                    .wrap(limiters.create_circuit_breaker.clone()),
            )
            .service(
                resource("/replies/{reply_id}")
                    .route(delete().to(feed_handler::delete_reply))
                    .wrap(middleware::csrf_middleware::CsrfMiddleware)
                    .wrap(limiters.delete_fair_use.clone())
                    .wrap(limiters.delete_circuit_breaker.clone()),
            ),
    );
}
