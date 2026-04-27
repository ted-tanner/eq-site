use std::{
    future::{Ready, ready},
    net::IpAddr,
    time::{Duration, Instant},
};

use ahash::RandomState as AHashRandomState;

use crate::{env, middleware::peer_ip};

pub mod rate_limiter_table;
pub mod strategies;

use rate_limiter_table as rate_limit_table;
use rate_limiter_table::CheckAndRecordResult;
use rate_limiter_table::RateLimiterTable;

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    error::ErrorTooManyRequests,
};

use futures::future::LocalBoxFuture;
use tokio::sync::RwLock;

pub trait RateLimiterStrategy {
    fn gen_key_and_shard_idx<const SHARDS: usize>(ip: IpAddr) -> (u128, u8);
    fn format_key_for_log(ip: IpAddr, key: u128) -> String;
}

pub use strategies::circuit_breaker::CircuitBreaker;
pub use strategies::fair_use::FairUse;

pub struct RateLimiter<STRATEGY: RateLimiterStrategy, const SHARDS: usize> {
    max_per_period: u64,
    period: Duration,
    clear_frequency: Duration,
    warn_every_over_limit: u32,
    limiter_tables: &'static [RwLock<RateLimiterTable<u128, AHashRandomState>>; SHARDS],
    name: &'static str,
    _phantom: std::marker::PhantomData<STRATEGY>,
}

impl<STRATEGY: RateLimiterStrategy, const SHARDS: usize> Clone for RateLimiter<STRATEGY, SHARDS> {
    fn clone(&self) -> Self {
        Self {
            max_per_period: self.max_per_period,
            period: self.period,
            clear_frequency: self.clear_frequency,
            warn_every_over_limit: self.warn_every_over_limit,
            limiter_tables: self.limiter_tables,
            name: self.name,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<STRATEGY: RateLimiterStrategy, const SHARDS: usize> RateLimiter<STRATEGY, SHARDS> {
    pub fn new(
        max_per_period: u64,
        period: Duration,
        clear_frequency: Duration,
        name: &'static str,
    ) -> Self {
        if period > clear_frequency {
            panic!("Period cannot be greater than clear frequency");
        }
        let max_period_ms = 14 * 24 * 60 * 60 * 1000u64;
        if period.as_millis() as u64 >= max_period_ms {
            panic!("RateLimiter period must be less than 14 days");
        }

        let warn_every_over_limit = env::CONF.rate_limiter_warn_every_over_limit;

        rate_limit_table::init_start();
        let limiter_tables =
            rate_limit_table::new_sharded_tables::<u128, AHashRandomState, SHARDS>();

        RateLimiter {
            max_per_period,
            period,
            clear_frequency,
            warn_every_over_limit,
            limiter_tables,
            name,
            _phantom: std::marker::PhantomData,
        }
    }
}

pub struct RateLimiterMiddleware<S, STRATEGY: RateLimiterStrategy, const SHARDS: usize> {
    service: S,
    limiter: RateLimiter<STRATEGY, SHARDS>,
    _phantom: std::marker::PhantomData<STRATEGY>,
}

impl<S, B, STRATEGY: RateLimiterStrategy, const SHARDS: usize> Service<ServiceRequest>
    for RateLimiterMiddleware<S, STRATEGY, SHARDS>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ip = peer_ip::resolve_peer_ip(req.request());

        let req_fut = self.service.call(req);

        let max_per_period = self.limiter.max_per_period;
        let period = self.limiter.period;
        let clear_frequency = self.limiter.clear_frequency;
        let warn_every = self.limiter.warn_every_over_limit;
        let limiter_tables = self.limiter.limiter_tables;
        let limiter_name = self.limiter.name;

        Box::pin(async move {
            let Some(ip_addr) = ip else {
                return req_fut.await;
            };

            let (key, distinguishing_octet) = STRATEGY::gen_key_and_shard_idx::<SHARDS>(ip_addr);

            let now = Instant::now();
            let now_millis = rate_limit_table::instant_to_millis_u32(now);

            let table_index = (distinguishing_octet as usize) % SHARDS;
            let shard = &limiter_tables[table_index];

            let result = rate_limit_table::check_and_record(
                shard,
                key,
                now,
                now_millis,
                max_per_period as u32,
                period,
                clear_frequency,
            )
            .await;

            if let CheckAndRecordResult::Blocked { count } = result {
                if warn_every != 0 {
                    let limit = max_per_period as u32;
                    let delta = count.saturating_sub(limit).saturating_sub(1);
                    if delta % warn_every == 0 {
                        let key_str = STRATEGY::format_key_for_log(ip_addr, key);
                        log::warn!(
                            "Rate-limited request (key={}, count={}, limit={}, warn_every={}, table_index={}, limiter_name={})",
                            key_str,
                            count,
                            limit,
                            warn_every,
                            table_index,
                            limiter_name
                        );
                    }
                }

                return Err(ErrorTooManyRequests(
                    "Too many requests. Please try again later.",
                ));
            }

            req_fut.await
        })
    }
}

impl<S, B, STRATEGY: RateLimiterStrategy, const SHARDS: usize> Transform<S, ServiceRequest>
    for RateLimiter<STRATEGY, SHARDS>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RateLimiterMiddleware<S, STRATEGY, SHARDS>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterMiddleware {
            service,
            limiter: self.clone(),
            _phantom: std::marker::PhantomData,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::CircuitBreaker;
    use super::FairUse;
    use super::RateLimiter;
    use crate::test_utils::rate_limiting::{
        SHARED_WARNINGS, init_shared_test_logger, lock_shared_warning_test,
    };
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};
    use std::net::SocketAddr;
    use std::time::Duration;

    fn peer_addr(s: &str) -> SocketAddr {
        SocketAddr::new(s.parse().unwrap(), 0)
    }
    use tokio::time::sleep;

    #[actix_web::test]
    async fn test_limiter_ipv4() {
        let limiter = RateLimiter::<CircuitBreaker, 16>::new(
            2,
            Duration::from_millis(10),
            Duration::from_millis(16),
            "GET /test",
        );

        let app =
            test::init_service(App::new().wrap(limiter).service(
                web::resource("/").to(|| async { HttpResponse::Ok().body("Hello world") }),
            ))
            .await;

        // Requests from different IPs in the same /24 subnet (127.0.0.0/24) should share the limit
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.2"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        // Throw in a request from a different subnet (same table/shard) to make sure it doesn't affect the limit
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.1.0.1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.99"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS); // Should hit the limit for the subnet

        // Other IP in a different /24 subnet
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.1.1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        sleep(Duration::from_millis(10)).await;

        // Period has expired, so we should be able to make another request from the same subnet
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.3"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.4"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.5"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        sleep(Duration::from_millis(2)).await;

        // Period has not expired
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.6"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        // Make a request from a new IP in a different subnet (and different table)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.15.1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.7"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        sleep(Duration::from_millis(6)).await;

        // This request should trigger a clear (different subnet, same table)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.2.0.1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        // Table has been cleared, so we should be able to make another request from the original subnet
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.8"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.9"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.10"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[actix_web::test]
    async fn test_limiter_ipv6() {
        let limiter = RateLimiter::<CircuitBreaker, 16>::new(
            2,
            Duration::from_millis(10),
            Duration::from_millis(16),
            "GET /test",
        );

        let app =
            test::init_service(App::new().wrap(limiter).service(
                web::resource("/").to(|| async { HttpResponse::Ok().body("Hello world") }),
            ))
            .await;

        // Requests from different IPs in the same /64 subnet (b24c:089b:7a21:1aff::/64) should share the limit
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::2"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        // Throw in a request from a different subnet (same table/shard) to make sure it doesn't affect the limit
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24d:089b:7a21:1aff::2"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::abcd"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS); // Should hit the limit for the subnet

        // Other IP in a different /64 subnet (change 4th segment)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1bff::1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        sleep(Duration::from_millis(10)).await;

        // Period has expired, so we should be able to make another request from the same subnet
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::3"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::4"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::5"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        sleep(Duration::from_millis(2)).await;

        // Period has not expired
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::6"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        // Make a request from a new IP in a different subnet (and different table)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aee::1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::7"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        sleep(Duration::from_millis(6)).await;

        // This request should trigger the clear (different subnet, same table)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1cff::1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        // Table has been cleared, so we should be able to make another request from the original subnet
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::8"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::9"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::OK);

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::a"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[actix_web::test]
    async fn test_limiter_warning_logging() {
        let _warning_test_guard = lock_shared_warning_test().await;
        init_shared_test_logger();
        // Clear warnings at the start of this test
        SHARED_WARNINGS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();

        let warn_every = crate::env::CONF.rate_limiter_warn_every_over_limit;
        if warn_every == 0 {
            eprintln!("Skipping test: warn_every_over_limit is 0 (warnings disabled)");
            return;
        }

        let limit = 2u32;
        let limiter = RateLimiter::<FairUse, 16>::new(
            limit as u64,
            Duration::from_secs(30),
            Duration::from_secs(60),
            "GET /test",
        );

        let app =
            test::init_service(App::new().wrap(limiter).service(
                web::resource("/").to(|| async { HttpResponse::Ok().body("Hello world") }),
            ))
            .await;

        // Warnings occur at: limit+1, limit+1+warn_every, limit+1+2*warn_every, limit+1+3*warn_every
        // To get 4 warnings, need count = limit + 1 + 3*warn_every
        let requests_needed = limit + 1 + (3 * warn_every);

        for i in 1..=limit {
            let req = test::TestRequest::default()
                .peer_addr(peer_addr("127.0.0.1"))
                .to_request();
            let status = match test::try_call_service(&app, req).await {
                Ok(res) => res.status(),
                Err(err) => err.as_response_error().status_code(),
            };
            assert_eq!(status, StatusCode::OK, "Request {} should be allowed", i);
        }

        for i in (limit + 1)..=requests_needed {
            let req = test::TestRequest::default()
                .peer_addr(peer_addr("127.0.0.1"))
                .to_request();
            let status = match test::try_call_service(&app, req).await {
                Ok(res) => res.status(),
                Err(err) => err.as_response_error().status_code(),
            };
            assert_eq!(
                status,
                StatusCode::TOO_MANY_REQUESTS,
                "Request {} should be blocked",
                i
            );
        }

        let warnings = SHARED_WARNINGS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let expected_warnings = 4;

        let warnings_for_limiter: Vec<&String> = warnings
            .iter()
            .filter(|w| w.contains("limiter_name=GET /test"))
            .collect();

        assert!(
            warnings_for_limiter.len() >= expected_warnings,
            "Expected at least {} warnings for limiter_name=GET /test (warn_every={}, limit={}, requests={}), got {} total warnings / {} matching",
            expected_warnings,
            warn_every,
            limit,
            requests_needed,
            warnings.len(),
            warnings_for_limiter.len()
        );

        for warning in warnings_for_limiter.iter() {
            assert!(
                warning.contains("Rate-limited request"),
                "Warning should contain 'Rate-limited request', got: {}",
                warning
            );
            assert!(
                warning.contains("key="),
                "Warning should contain 'key=', got: {}",
                warning
            );
            assert!(
                warning.contains(&format!("warn_every={}", warn_every)),
                "Warning should contain 'warn_every={}', got: {}",
                warn_every,
                warning
            );
            assert!(
                warning.contains(&format!("limit={}", limit)),
                "Warning should contain 'limit={}', got: {}",
                limit,
                warning
            );
            assert!(
                warning.contains("limiter_name=GET /test"),
                "Warning should contain 'limiter_name=GET /test', got: {}",
                warning
            );
        }
    }
}
