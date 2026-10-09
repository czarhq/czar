use super::*;

#[tokio::test]
async fn token_mode_accepts_bearer_or_session_cookie() {
    let auth = ApiAuth::token("s3cret");
    let session = session_value("s3cret");
    assert!(
        auth.allows(&headers(&[("authorization", "Bearer s3cret")]))
            .await
    );
    assert!(
        auth.allows(&headers(&[(
            "cookie",
            &format!("theme=dark; {SESSION_COOKIE}={session}")
        )]))
        .await
    );
    assert!(!auth.allows(&headers(&[])).await);
    assert!(
        !auth
            .allows(&headers(&[("authorization", "Bearer wrong")]))
            .await
    );
    assert!(
        !auth
            .allows(&headers(&[("authorization", "Basic s3cret")]))
            .await
    );
    // The raw token is not a valid cookie value.
    assert!(
        !auth
            .allows(&headers(&[("cookie", &format!("{SESSION_COOKIE}=s3cret"))]))
            .await
    );
}

#[test]
fn loopback_defaults_to_none_and_a_token_implies_token_mode() {
    assert_eq!(
        args(None, None).resolve(loopback()).unwrap().mode(),
        AuthMode::None
    );
    assert_eq!(
        args(None, Some("s3cret")).resolve(public()).unwrap().mode(),
        AuthMode::Token
    );
}

#[test]
fn public_address_needs_an_explicit_mode() {
    let err = args(None, None).resolve(public()).unwrap_err().to_string();
    assert!(err.contains("--auth"), "{err}");
    assert_eq!(
        args(Some(AuthMode::None), None)
            .resolve(public())
            .unwrap()
            .mode(),
        AuthMode::None
    );
}

#[test]
fn rejects_options_that_do_not_fit_the_mode() {
    for bad in [
        args(Some(AuthMode::Token), None),
        args(Some(AuthMode::None), Some("s3cret")),
    ] {
        assert!(bad.resolve(public()).is_err(), "{bad:?}");
    }
}

#[test]
fn cookie_is_secure_only_over_https() {
    assert!(is_https(&headers(&[(
        "origin",
        "https://czar.example.com"
    )])));
    assert!(is_https(&headers(&[("x-forwarded-proto", "https")])));
    assert!(!is_https(&headers(&[("origin", "http://127.0.0.1:4000")])));
}

// Helpers

fn args(mode: Option<AuthMode>, token: Option<&str>) -> AuthArgs {
    AuthArgs {
        mode,
        auth_token: token.map(str::to_string),
    }
}

fn loopback() -> SocketAddr {
    "127.0.0.1:4000".parse().unwrap()
}

fn public() -> SocketAddr {
    "0.0.0.0:4000".parse().unwrap()
}

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    map
}
