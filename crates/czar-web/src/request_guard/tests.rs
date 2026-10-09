use super::*;

#[test]
fn allows_requests_without_origin() {
    let guard = RequestGuard::new(true, &[]);
    assert!(guard.check(&headers(Some("127.0.0.1:4000"), None)).is_ok());
    assert!(guard.check(&headers(None, None)).is_ok());
}

#[test]
fn allows_loopback_and_same_origin() {
    let guard = RequestGuard::new(false, &[]);
    for origin in [
        "http://localhost:5175",
        "http://127.0.0.1:4000",
        "http://[::1]:4000",
    ] {
        assert!(
            guard
                .check(&headers(Some("api:4000"), Some(origin)))
                .is_ok()
        );
    }
    assert!(
        guard
            .check(&headers(
                Some("czar.example.com"),
                Some("https://czar.example.com")
            ))
            .is_ok()
    );
}

#[test]
fn rejects_cross_site_origin() {
    let guard = RequestGuard::new(false, &[]);
    for origin in ["https://evil.example", "null"] {
        assert!(
            guard
                .check(&headers(Some("127.0.0.1:4000"), Some(origin)))
                .is_err()
        );
    }
}

#[test]
fn rejects_look_alike_loopback_origins() {
    let guard = RequestGuard::new(false, &[]);
    // A domain that starts with "localhost", and a user name before `@`.
    for origin in [
        "http://localhost.evil.example",
        "http://localhost@evil.example",
    ] {
        assert!(
            guard
                .check(&headers(Some("127.0.0.1:4000"), Some(origin)))
                .is_err()
        );
    }
}

#[test]
fn rejects_rebound_host_on_loopback() {
    let guard = RequestGuard::new(true, &[]);
    let rebound = headers(Some("evil.example:4000"), Some("http://evil.example:4000"));
    assert!(guard.check(&rebound).is_err());
    assert!(RequestGuard::new(false, &[]).check(&rebound).is_ok());
}

#[test]
fn allowed_origins_extend_origin_and_host_checks() {
    let guard = RequestGuard::new(true, &["https://czar.example.com/".to_string()]);
    assert!(
        guard
            .check(&headers(
                Some("czar.example.com"),
                Some("https://czar.example.com")
            ))
            .is_ok()
    );
    assert!(
        guard
            .check(&headers(
                Some("127.0.0.1:4000"),
                Some("https://czar.example.com")
            ))
            .is_ok()
    );
    assert!(
        guard
            .check(&headers(
                Some("127.0.0.1:4000"),
                Some("http://czar.example.com")
            ))
            .is_err()
    );
}

#[test]
fn parse_allowed_origin_requires_an_http_origin() {
    assert_eq!(
        parse_allowed_origin("https://czar.example.com/").unwrap(),
        "https://czar.example.com"
    );
    assert!(parse_allowed_origin("czar.example.com").is_err());
    assert!(parse_allowed_origin("ftp://czar.example.com").is_err());
}

// Helpers

fn headers(host: Option<&str>, origin: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(host) = host {
        headers.insert(header::HOST, host.parse().unwrap());
    }
    if let Some(origin) = origin {
        headers.insert(header::ORIGIN, origin.parse().unwrap());
    }
    headers
}
