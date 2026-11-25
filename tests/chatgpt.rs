use super::*;
use std::time::Duration;

#[test]
fn model_route_rejects_a_token_from_a_different_account() {
    assert_eq!(
        token_for_expected_registration(
            Some(("account-a".into(), "test-token".into())),
            "account-a"
        )
        .unwrap(),
        Some("test-token".into())
    );
    assert!(token_for_expected_registration(
        Some(("account-b".into(), "test-token".into())),
        "account-a"
    )
    .is_err());
    assert_eq!(
        token_for_expected_registration(None, "account-a").unwrap(),
        None
    );
}

fn registration(scopes: &[&str]) -> Registration {
    Registration {
        id: "saved-id".into(),
        client_id: "oaiapp-issued".into(),
        subject: "subject".into(),
        issuer: "https://auth.openai.com".into(),
        name: Some("Reader".into()),
        email: Some("reader@example.invalid".into()),
        credentials: Some(CredentialSet {
            access_token: "access-secret".into(),
            refresh_token: Some("refresh-secret".into()),
            id_token: "id-secret".into(),
            expires_at: now() + 3600,
            scopes: scopes.iter().map(|s| (*s).into()).collect(),
        }),
    }
}

#[test]
fn account_status_serializes_camel_case_and_never_exposes_credentials() {
    let mut data = StoreData::default();
    data.active_id = Some("saved-id".into());
    data.registrations
        .push(registration(&["openid", "chatgpt.tokens.use.direct"]));
    let status = serde_json::to_value(status_from_data(&data, true)).unwrap();
    assert_eq!(status["state"], "signedIn");
    assert_eq!(status["signInPending"], true);
    assert_eq!(status["account"]["registrationId"], "saved-id");
    assert_eq!(status["account"]["planGranted"], true);
    assert_eq!(
        status["registrations"][0]["email"],
        "reader@example.invalid"
    );
    let serialized = status.to_string();
    assert!(!serialized.contains("access-secret"));
    assert!(!serialized.contains("refresh-secret"));
    assert!(!serialized.contains("id-secret"));
}

#[test]
fn signed_out_status_keeps_account_explicitly_null() {
    let status = serde_json::to_value(status_from_data(&StoreData::default(), false)).unwrap();
    assert_eq!(status["state"], "signedOut");
    assert!(status["account"].is_null());
}

#[test]
fn plan_permission_comes_only_from_the_granted_scope() {
    let mut data = StoreData::default();
    data.active_id = Some("saved-id".into());
    data.registrations
        .push(registration(&["openid", "profile"]));
    assert_eq!(
        status_from_data(&data, false).account.unwrap().plan_granted,
        false
    );
    data.registrations[0]
        .credentials
        .as_mut()
        .unwrap()
        .scopes
        .push("chatgpt.tokens.use.direct".into());
    assert_eq!(
        status_from_data(&data, false).account.unwrap().plan_granted,
        true
    );
}

#[test]
fn oidc_endpoints_must_remain_on_the_trusted_https_origin() {
    assert!(trusted_endpoint("https://auth.openai.com/oauth2/jwks"));
    for url in [
        "http://auth.openai.com/oauth2/jwks",
        "https://auth.openai.com.evil.invalid/oauth2/jwks",
        "https://evil.invalid/oauth2/jwks",
        "https://auth.openai.com:444/oauth2/jwks",
    ] {
        assert!(!trusted_endpoint(url), "{url}");
    }
}

#[test]
fn invalid_jwt_and_oidc_claims_are_rejected_without_network_calls() {
    assert!(decode_header("not-a-jwt").is_err());
    let valid = IdClaims {
        iss: "https://auth.openai.com".into(),
        sub: "subject".into(),
        aud: serde_json::json!(["other-client", "expected-client"]),
        exp: 200,
        nonce: "expected-nonce".into(),
        email: None,
        name: None,
    };
    assert!(valid_claims(
        &valid,
        "expected-client",
        "expected-nonce",
        100
    ));
    let mut wrong_nonce = valid;
    wrong_nonce.nonce = "wrong".into();
    assert!(!valid_claims(
        &wrong_nonce,
        "expected-client",
        "expected-nonce",
        100
    ));
    wrong_nonce.nonce = "expected-nonce".into();
    assert!(!valid_claims(
        &wrong_nonce,
        "missing-client",
        "expected-nonce",
        100
    ));
    wrong_nonce.aud = serde_json::json!("expected-client");
    wrong_nonce.exp = 50;
    assert!(!valid_claims(
        &wrong_nonce,
        "expected-client",
        "expected-nonce",
        100
    ));
}

#[test]
fn expired_identity_token_is_not_reported_as_a_signed_in_plan_session() {
    let mut data = StoreData::default();
    data.active_id = Some("saved-id".into());
    let mut row = registration(&["chatgpt.tokens.use.direct"]);
    row.credentials.as_mut().unwrap().expires_at = now().saturating_sub(1);
    data.registrations.push(row);
    let status = status_from_data(&data, false);
    assert_eq!(status.state, AccountState::ReauthRequired);
    assert!(!status.account.unwrap().plan_granted);
}

#[test]
fn refresh_errors_separate_terminal_credentials_from_retryable_failures() {
    for code in [
        "invalid_grant",
        "invalid_client",
        "invalid_refresh_token",
        "refresh_token_reused",
    ] {
        assert!(terminal_refresh_error(code), "{code}");
    }
    for retryable in ["", "temporarily_unavailable", "server_error"] {
        assert!(!terminal_refresh_error(retryable), "{retryable}");
    }
    let mut row = registration(&["openid"]);
    let previous = row.credentials.clone();
    assert!(!clear_on_terminal_refresh_error(
        &mut row,
        Some("server_error")
    ));
    assert_eq!(
        row.credentials.as_ref().unwrap().refresh_token,
        previous.unwrap().refresh_token
    );
    assert!(clear_on_terminal_refresh_error(
        &mut row,
        Some("invalid_grant")
    ));
    assert!(row.credentials.is_none());
}

#[test]
fn callback_ignores_wrong_state_and_accepts_the_later_matching_response() {
    assert!(callback_from_request_line(
        "GET /auth/callback?state=wrong&code=attacker HTTP/1.1\r\n",
        "expected"
    )
    .unwrap()
    .is_none());
    let result = callback_from_request_line(
        "GET /auth/callback?state=expected&code=real-code&client_id=oaiapp-issued HTTP/1.1\r\n",
        "expected",
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.code, "real-code");
    assert_eq!(result.client_id.as_deref(), Some("oaiapp-issued"));
    assert!(callback_from_request_line("malformed", "expected")
        .unwrap()
        .is_none());
}

#[test]
fn callback_wait_can_be_cancelled_and_times_out() {
    assert_eq!(
        callback_wait_state(true, Duration::ZERO, Duration::from_secs(1)),
        CallbackWait::Cancelled
    );
    assert_eq!(
        callback_wait_state(false, Duration::from_secs(2), Duration::from_secs(1)),
        CallbackWait::TimedOut
    );
    assert_eq!(
        callback_wait_state(false, Duration::ZERO, Duration::from_secs(1)),
        CallbackWait::Waiting
    );
}

#[cfg(windows)]
#[test]
fn dpapi_protects_and_restores_fake_test_bytes() {
    let original = b"fake credential bytes only";
    let protected = protect(original).unwrap();
    assert_ne!(protected, original);
    assert_eq!(unprotect(&protected).unwrap(), original);
}
