use std::net::IpAddr;

use actix_web::{Error, FromRequest, HttpRequest, dev::Payload};
use futures::future::{Ready, ready};

use crate::env;

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct PeerIp(pub Option<IpAddr>);

impl PeerIp {
    #[allow(dead_code)]
    pub fn into_option(self) -> Option<IpAddr> {
        self.0
    }
}

impl FromRequest for PeerIp {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(PeerIp(resolve_peer_ip(req))))
    }
}

pub fn resolve_peer_ip(req: &HttpRequest) -> Option<IpAddr> {
    if env::CONF.peer_ip_use_x_forwarded_for {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(client_ip_from_xff)
            .or_else(|| req.peer_addr().map(|addr| addr.ip()))
    } else {
        req.peer_addr().map(|addr| addr.ip())
    }
}

pub fn client_ip_from_xff(xff: &str) -> Option<IpAddr> {
    let left_most_ip = xff.split_once(',').map(|(a, _)| a).unwrap_or(xff);
    left_most_ip.trim().parse::<IpAddr>().ok()
}

#[cfg(test)]
mod tests {
    use super::{client_ip_from_xff, resolve_peer_ip};
    use actix_web::test::TestRequest;
    use std::net::{IpAddr, SocketAddr};
    use std::sync::{LazyLock, Mutex};

    static PEER_IP_CONFIG_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn peer_addr(s: &str) -> SocketAddr {
        SocketAddr::new(s.parse().unwrap(), 0)
    }

    #[test]
    fn test_client_ip_from_xff_uses_left_most_ip() {
        assert_eq!(
            client_ip_from_xff("203.0.113.8, 10.0.0.1"),
            Some("203.0.113.8".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn test_client_ip_from_xff_rejects_invalid_header() {
        assert_eq!(client_ip_from_xff("not-an-ip"), None);
    }

    #[test]
    fn test_resolve_peer_ip_without_proxy_uses_peer_addr() {
        let _guard = PEER_IP_CONFIG_LOCK.lock().unwrap();
        crate::env::set_peer_ip_use_x_forwarded_for_for_test(false);
        let req = TestRequest::default()
            .peer_addr(peer_addr("2001:db8::7"))
            .to_srv_request();
        assert_eq!(
            resolve_peer_ip(req.request()),
            Some("2001:db8::7".parse().unwrap())
        );
    }

    #[test]
    fn test_resolve_peer_ip_with_proxy_uses_xff() {
        let _guard = PEER_IP_CONFIG_LOCK.lock().unwrap();
        crate::env::set_peer_ip_use_x_forwarded_for_for_test(true);
        let req = TestRequest::default()
            .peer_addr(peer_addr("127.0.0.1"))
            .insert_header(("x-forwarded-for", "198.51.100.12, 127.0.0.1"))
            .to_http_request();
        assert_eq!(
            resolve_peer_ip(&req),
            Some("198.51.100.12".parse().unwrap())
        );
    }
}
