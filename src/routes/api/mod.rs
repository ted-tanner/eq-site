use actix_web::web::*;

use crate::env::CONF;
use crate::handlers::{admin_handler, public_handler};
use crate::middleware;
use crate::middleware::{CircuitBreaker, FairUse, RateLimiter};

mod admin;
mod auth;
mod feed;
mod notifications;
mod public;

#[derive(Clone)]
pub struct RateLimiters {
    pub light_auth_fair_use: RateLimiter<FairUse, 32>,
    pub heavy_auth_fair_use: RateLimiter<FairUse, 32>,
    pub create_fair_use: RateLimiter<FairUse, 32>,
    pub post_fair_use: RateLimiter<FairUse, 32>,
    pub survey_response_fair_use: RateLimiter<FairUse, 32>,
    pub read_fair_use: RateLimiter<FairUse, 32>,
    pub update_fair_use: RateLimiter<FairUse, 32>,
    pub delete_fair_use: RateLimiter<FairUse, 32>,
    pub light_auth_circuit_breaker: RateLimiter<CircuitBreaker, 32>,
    pub heavy_auth_circuit_breaker: RateLimiter<CircuitBreaker, 32>,
    pub create_circuit_breaker: RateLimiter<CircuitBreaker, 32>,
    pub read_circuit_breaker: RateLimiter<CircuitBreaker, 32>,
    pub update_circuit_breaker: RateLimiter<CircuitBreaker, 32>,
    pub delete_circuit_breaker: RateLimiter<CircuitBreaker, 32>,
}

impl Default for RateLimiters {
    fn default() -> Self {
        Self {
            light_auth_fair_use: RateLimiter::new(
                CONF.light_auth_fair_use_limiter_max_per_period,
                CONF.light_auth_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "light_auth_fair_use",
            ),
            heavy_auth_fair_use: RateLimiter::new(
                CONF.heavy_auth_fair_use_limiter_max_per_period,
                CONF.heavy_auth_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "heavy_auth_fair_use",
            ),
            create_fair_use: RateLimiter::new(
                CONF.create_fair_use_limiter_max_per_period,
                CONF.create_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "create_fair_use",
            ),
            post_fair_use: RateLimiter::new(
                CONF.post_fair_use_limiter_max_per_period,
                CONF.post_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "post_fair_use",
            ),
            survey_response_fair_use: RateLimiter::new(
                CONF.survey_response_fair_use_limiter_max_per_period,
                CONF.survey_response_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "survey_response_fair_use",
            ),
            read_fair_use: RateLimiter::new(
                CONF.read_fair_use_limiter_max_per_period,
                CONF.read_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "read_fair_use",
            ),
            update_fair_use: RateLimiter::new(
                CONF.update_fair_use_limiter_max_per_period,
                CONF.update_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "update_fair_use",
            ),
            delete_fair_use: RateLimiter::new(
                CONF.delete_fair_use_limiter_max_per_period,
                CONF.delete_fair_use_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "delete_fair_use",
            ),
            light_auth_circuit_breaker: RateLimiter::new(
                CONF.light_auth_circuit_breaker_limiter_max_per_period,
                CONF.light_auth_circuit_breaker_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "light_auth_circuit_breaker",
            ),
            heavy_auth_circuit_breaker: RateLimiter::new(
                CONF.heavy_auth_circuit_breaker_limiter_max_per_period,
                CONF.heavy_auth_circuit_breaker_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "heavy_auth_circuit_breaker",
            ),
            create_circuit_breaker: RateLimiter::new(
                CONF.create_circuit_breaker_limiter_max_per_period,
                CONF.create_circuit_breaker_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "create_circuit_breaker",
            ),
            read_circuit_breaker: RateLimiter::new(
                CONF.read_circuit_breaker_limiter_max_per_period,
                CONF.read_circuit_breaker_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "read_circuit_breaker",
            ),
            update_circuit_breaker: RateLimiter::new(
                CONF.update_circuit_breaker_limiter_max_per_period,
                CONF.update_circuit_breaker_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "update_circuit_breaker",
            ),
            delete_circuit_breaker: RateLimiter::new(
                CONF.delete_circuit_breaker_limiter_max_per_period,
                CONF.delete_circuit_breaker_limiter_period,
                CONF.rate_limiter_clear_frequency,
                "delete_circuit_breaker",
            ),
        }
    }
}

pub fn configure(cfg: &mut ServiceConfig) {
    let limiters = RateLimiters::default();
    cfg.service(
        scope("/api")
            .service(
                resource("/survey-responses")
                    .route(
                        get()
                            .to(admin_handler::list_survey_responses)
                            .wrap(limiters.read_fair_use.clone())
                            .wrap(limiters.read_circuit_breaker.clone()),
                    )
                    .route(
                        post()
                            .to(public_handler::create_survey_response)
                            .wrap(middleware::csrf_middleware::CsrfMiddleware)
                            .wrap(limiters.survey_response_fair_use.clone())
                            .wrap(limiters.create_circuit_breaker.clone()),
                    ),
            )
            .configure(|cfg| auth::configure(cfg, limiters.clone()))
            .configure(|cfg| feed::configure(cfg, limiters.clone()))
            .configure(|cfg| notifications::configure(cfg, limiters.clone()))
            .configure(|cfg| public::configure(cfg, limiters.clone()))
            .configure(|cfg| admin::configure(cfg, limiters)),
    );
}
