use super::*;

fn input(base_url: &str) -> ProviderInput {
    ProviderInput {
        id: None,
        name: "Example".into(),
        base_url: base_url.into(),
        api_key: Some("test-placeholder".into()),
        models: vec![" model-a ".into(), "".into()],
    }
}

#[test]
fn base_url_requires_an_api_root_address() {
    assert_eq!(
        normalize_base_url(" https://example.invalid:8443/api/v1/ ").unwrap(),
        "https://example.invalid:8443/api/v1"
    );
    assert_eq!(
        normalize_base_url("http://localhost:8080/").unwrap(),
        "http://localhost:8080"
    );
    for invalid in [
        "example.invalid",
        "https:///v1",
        "https://",
        "file:///tmp",
        "https://user:pass@example.invalid",
        "https://example.invalid?token=sample",
        "https://example.invalid/#v1",
        "https://@example.invalid",
    ] {
        assert!(normalize_base_url(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn saving_keeps_and_replaces_keys_without_returning_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("providers.json");
    let mut config = read(&path).unwrap();
    upsert(&mut config, input("https://example.invalid/v1")).unwrap();
    write(&path, &config).unwrap();
    let mut config = read(&path).unwrap();
    let id = config.providers[0].id.clone();
    let mut edit = input("https://example.invalid/v2");
    edit.id = Some(id.clone());
    edit.api_key = Some(" ".into());
    upsert(&mut config, edit).unwrap();
    assert_eq!(config.providers[0].api_key, "test-placeholder");
    let mut edit = input("https://example.invalid/v2");
    edit.id = Some(id.clone());
    edit.api_key = Some("replacement-placeholder".into());
    upsert(&mut config, edit).unwrap();
    write(&path, &config).unwrap();
    let reloaded = read(&path).unwrap();
    assert_eq!(reloaded.providers[0].id, id);
    assert_eq!(reloaded.providers[0].api_key, "replacement-placeholder");
    assert_eq!(reloaded.providers[0].models, ["model-a"]);
    let summaries = serde_json::to_value(summarize(reloaded)).unwrap();
    assert_eq!(summaries[0]["hasKey"], true);
    assert!(summaries[0].get("apiKey").is_none());
}

#[test]
fn does_not_read_an_unknown_config_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("providers.json");
    fs::write(&path, br#"{"version":2,"providers":[]}"#).unwrap();
    assert!(read(&path).is_err());
}
