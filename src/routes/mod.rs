pub mod api;

use actix_web::web::ServiceConfig;

use api::RateLimiters;

pub fn configure_with_limiters(cfg: &mut ServiceConfig, limiters: RateLimiters) {
    api::configure_with_limiters(cfg, limiters);
}
