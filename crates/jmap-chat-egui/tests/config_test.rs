use jmap_chat::error::ClientError;
use jmap_chat_egui::config::Config;

/// Construct a Config directly (bypassing clap argv parsing) so tests are
/// deterministic and do not depend on process environment.
fn base_config() -> Config {
    Config {
        server_url: "http://x".into(),
        bearer_token: None,
        basic_user: None,
        basic_pass: None,
        ca_cert: None,
    }
}

/// Oracle: bearer-only config produces a working auth provider.
/// The resulting provider must return a non-None auth header containing "Bearer".
#[test]
fn parse_bearer_only() {
    let cfg = Config {
        bearer_token: Some("tok123".into()),
        ..base_config()
    };
    let provider = cfg
        .auth_provider()
        .expect("bearer-only config must produce Ok");
    let header = provider.auth_header();
    assert!(
        header.is_some(),
        "BearerAuth must return an Authorization header"
    );
    let (_, value) = header.unwrap();
    let value_str = value.to_str().expect("header value must be valid ASCII");
    assert!(
        value_str.starts_with("Bearer "),
        "header value must start with 'Bearer ', got: {value_str}"
    );
}

/// Oracle: basic-only config produces a working auth provider.
/// The resulting provider must return a non-None auth header containing "Basic".
#[test]
fn parse_basic_only() {
    let cfg = Config {
        basic_user: Some("alice".into()),
        basic_pass: Some("secret".into()),
        ..base_config()
    };
    let provider = cfg
        .auth_provider()
        .expect("basic-only config must produce Ok");
    let header = provider.auth_header();
    assert!(
        header.is_some(),
        "BasicAuth must return an Authorization header"
    );
    let (_, value) = header.unwrap();
    let value_str = value.to_str().expect("header value must be valid ASCII");
    assert!(
        value_str.starts_with("Basic "),
        "header value must start with 'Basic ', got: {value_str}"
    );
}

/// Oracle: no auth flags produces NoneAuth, which returns no Authorization header.
#[test]
fn parse_no_auth() {
    let cfg = base_config();
    let provider = cfg.auth_provider().expect("no-auth config must produce Ok");
    assert!(
        provider.auth_header().is_none(),
        "NoneAuth must return no Authorization header"
    );
}

/// Oracle: supplying both bearer_token and basic_user must be rejected.
#[test]
fn parse_conflicting_auth() {
    let cfg = Config {
        bearer_token: Some("tok".into()),
        basic_user: Some("alice".into()),
        basic_pass: Some("secret".into()),
        ..base_config()
    };
    let result = cfg.auth_provider();
    match result {
        Err(ClientError::InvalidArgument(msg)) => {
            assert!(
                msg.contains("mutually exclusive") || msg.contains("exclusive"),
                "error must mention mutual exclusion, got: {msg}"
            );
        }
        Ok(_) => panic!("conflicting auth flags must be rejected"),
        Err(e) => panic!("expected InvalidArgument, got: {e}"),
    }
}

/// Oracle: basic_user without basic_pass must be rejected.
#[test]
fn parse_missing_basic_pass() {
    let cfg = Config {
        basic_user: Some("alice".into()),
        basic_pass: None,
        ..base_config()
    };
    let result = cfg.auth_provider();
    match result {
        Err(ClientError::InvalidArgument(msg)) => {
            assert!(
                msg.contains("basic-user") || msg.contains("basic_user") || msg.contains("both"),
                "error must mention both flags being required, got: {msg}"
            );
        }
        Ok(_) => panic!("missing basic_pass must be rejected"),
        Err(e) => panic!("expected InvalidArgument, got: {e}"),
    }
}

/// Oracle: basic_pass without basic_user must also be rejected.
#[test]
fn parse_missing_basic_user() {
    let cfg = Config {
        basic_user: None,
        basic_pass: Some("secret".into()),
        ..base_config()
    };
    let result = cfg.auth_provider();
    match result {
        Err(ClientError::InvalidArgument(_)) => {}
        Ok(_) => panic!("missing basic_user must be rejected"),
        Err(e) => panic!("expected InvalidArgument, got: {e}"),
    }
}
