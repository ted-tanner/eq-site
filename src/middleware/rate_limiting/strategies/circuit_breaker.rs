use std::net::IpAddr;

use super::super::RateLimiterStrategy;

/// Generates subnet-based keys (IPv4 /24 and IPv6 /64).
#[derive(Clone, Copy)]
pub struct CircuitBreaker;

impl RateLimiterStrategy for CircuitBreaker {
    #[inline]
    fn gen_key_and_shard_idx<const SHARDS: usize>(ip: IpAddr) -> (u128, u8) {
        match ip {
            IpAddr::V4(ip) => {
                let octets = ip.octets();
                let distinguishing_octet = octets[2];
                let key = u32::from_be_bytes([octets[0], octets[1], octets[2], 0]) as u128;
                (key, distinguishing_octet)
            }
            IpAddr::V6(ip) => {
                let octets = ip.octets();
                let distinguishing_octet = octets[7];
                let upper = u64::from_be_bytes([
                    octets[0], octets[1], octets[2], octets[3], octets[4], octets[5], octets[6],
                    octets[7],
                ]);
                let key = (upper as u128) << 64;
                (key, distinguishing_octet)
            }
        }
    }

    fn format_key_for_log(ip: IpAddr, _key: u128) -> String {
        match ip {
            IpAddr::V4(ip) => {
                let octets = ip.octets();
                format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(ip) => {
                let segments = ip.segments();
                format!(
                    "{:x}:{:x}:{:x}:{:x}::/64",
                    segments[0], segments[1], segments[2], segments[3]
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::RateLimiter;
    use super::CircuitBreaker;
    use crate::test_utils::rate_limiting::{
        SHARED_WARNINGS, init_shared_test_logger, lock_shared_warning_test,
    };
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::time::sleep;

    fn peer_addr(s: &str) -> SocketAddr {
        SocketAddr::new(s.parse().unwrap(), 0)
    }

    #[actix_web::test]
    async fn test_circuit_breaker_limiter_ipv4_subnet() {
        let limiter = RateLimiter::<CircuitBreaker, 16>::new(
            2,
            Duration::from_millis(10),
            Duration::from_millis(16),
            "circuit_breaker_ipv4",
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
        assert_eq!(
            status,
            StatusCode::OK,
            "First request from subnet should be allowed"
        );

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.2"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Second request from same subnet should be allowed"
        );

        // Third request from same subnet should be blocked (limit is 2)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.99"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Third request from same subnet should be blocked"
        );

        // Request from different /24 subnet should be allowed
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.1.1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Request from different subnet should be allowed"
        );

        sleep(Duration::from_millis(10)).await;

        // Period has expired, so we should be able to make requests from the original subnet again
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.3"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Request after period expiration should be allowed"
        );

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.4"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Second request after period expiration should be allowed"
        );

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("127.0.0.5"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Third request after period expiration should be blocked"
        );
    }

    #[actix_web::test]
    async fn test_circuit_breaker_limiter_ipv6_subnet() {
        let limiter = RateLimiter::<CircuitBreaker, 16>::new(
            2,
            Duration::from_millis(10),
            Duration::from_millis(16),
            "circuit_breaker_ipv6",
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
        assert_eq!(
            status,
            StatusCode::OK,
            "First request from subnet should be allowed"
        );

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::2"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Second request from same subnet should be allowed"
        );

        // Third request from same subnet should be blocked (limit is 2)
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::abcd"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Third request from same subnet should be blocked"
        );

        // Request from different /64 subnet should be allowed
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1bff::1"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Request from different subnet should be allowed"
        );

        sleep(Duration::from_millis(10)).await;

        // Period has expired, so we should be able to make requests from the original subnet again
        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::3"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Request after period expiration should be allowed"
        );

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::4"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::OK,
            "Second request after period expiration should be allowed"
        );

        let req = test::TestRequest::default()
            .peer_addr(peer_addr("b24c:089b:7a21:1aff::5"))
            .to_request();
        let status = match test::try_call_service(&app, req).await {
            Ok(res) => res.status(),
            Err(err) => err.as_response_error().status_code(),
        };
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Third request after period expiration should be blocked"
        );
    }

    #[actix_web::test]
    async fn test_circuit_breaker_limiter_warning_logging() {
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
        let limiter = RateLimiter::<CircuitBreaker, 16>::new(
            limit as u64,
            Duration::from_secs(30),
            Duration::from_secs(60),
            "circuit_breaker_test",
        );

        let app =
            test::init_service(App::new().wrap(limiter).service(
                web::resource("/").to(|| async { HttpResponse::Ok().body("Hello world") }),
            ))
            .await;

        // Warnings occur at: limit+1, limit+1+warn_every, limit+1+2*warn_every, limit+1+3*warn_every
        // To get 4 warnings, need count = limit + 1 + 3*warn_every
        let requests_needed = limit + 1 + (3 * warn_every);

        // Make requests from same subnet (they share the limit).
        // Use 127.0.0.1 for all - IPv4 octets must be 0-255, so avoid 127.0.0.{256+}.
        for _ in 1..=limit {
            let req = test::TestRequest::default()
                .peer_addr(peer_addr("127.0.0.1"))
                .to_request();
            let status = match test::try_call_service(&app, req).await {
                Ok(res) => res.status(),
                Err(err) => err.as_response_error().status_code(),
            };
            assert_eq!(status, StatusCode::OK, "Request should be allowed");
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
            .filter(|w| w.contains("limiter_name=circuit_breaker_test"))
            .collect();

        assert!(
            warnings_for_limiter.len() >= expected_warnings,
            "Expected at least {} warnings for limiter_name=circuit_breaker_test (warn_every={}, limit={}, requests={}), got {} total warnings / {} matching",
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
                warning.contains("/24") || warning.contains("/64"),
                "Warning should contain subnet mask, got: {}",
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
                warning.contains("limiter_name=circuit_breaker_test"),
                "Warning should contain limiter name, got: {}",
                warning
            );
        }
    }
}
