use super::*;

fn configured(text: &str) -> (tempfile::TempDir, ProviderOptions) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    std::fs::write(&path, text).unwrap();
    (
        directory,
        ProviderOptions {
            config: Some(path),
            key: Some("synthetic-secret".into()),
            ..Default::default()
        },
    )
}

#[test]
fn unconfigured_provider_error_includes_its_public_name_not_the_key() {
    let (_directory, mut options) = configured("[historical]\n");
    options.provider = Some("invalid_provider".into());
    let error = options.resolve().unwrap_err();
    assert_eq!(error.code, "SOURCE_CONFIGURATION");
    assert_eq!(
        error.message,
        "historical provider: \"invalid_provider\" is not configured"
    );
    assert!(!error.to_string().contains("synthetic-secret"));
}

#[test]
fn named_provider_and_custom_endpoint_remain_secret_even_in_debug() {
    let (_directory, options) = configured("[historical]\nprovider='archive'\n[historical.providers.archive]\nadapter='alchemy'\nurl='https://example.invalid/v2/{key}'\n");
    let provider = options.resolve().unwrap();
    assert_eq!(provider.name, "archive");
    assert_eq!(provider.adapter, "alchemy");
    let debug = format!("{provider:?}");
    assert!(!debug.contains("synthetic-secret"));
    assert!(!debug.contains("example.invalid"));
    assert_eq!(
        provider.transport_parts(),
        (
            "https://example.invalid/v2/synthetic-secret".into(),
            "synthetic-secret".into()
        )
    );
}

#[test]
fn invalid_config_never_echoes_input_and_never_selects_another_adapter() {
    for text in [
        "[historical.providers.alchemy]\nadapter='txlab'\n",
        "[historical.providers.alchemy]\nkey='synthetic-secret'\n",
        "[historical]\nprovider='missing'\n",
        "[historical]\nprovider='synthetic-secret\n",
        "[historical.providers.alchemy]\nkey_env='X'\nkey_file='key'\n",
        "[historical.providers.alchemy]\nurl='http://example.invalid/{key}'\n",
        "[historical.providers.alchemy]\nurl='https://user@example.invalid/{key}'\n",
        "[historical.providers.alchemy]\nurl='https://example.invalid/{key}#{key}'\n",
        "[historical.providers.alchemy]\nurl='https://example.invalid'\n",
    ] {
        let (_directory, options) = configured(text);
        let error = options.resolve().unwrap_err();
        assert_eq!(error.code, "SOURCE_CONFIGURATION");
        assert!(!error.to_string().contains("synthetic-secret"));
    }
}

#[test]
fn credentials_are_strict_and_explicit_missing_config_is_not_ignored() {
    for key in [
        "",
        "key with spaces",
        "https://example.invalid/key",
        "$(echo key)",
    ] {
        let (_directory, mut options) = configured("[historical]\n");
        options.key = Some(key.into());
        assert_eq!(options.resolve().unwrap_err().code, "SOURCE_CONFIGURATION");
    }
    let directory = tempfile::tempdir().unwrap();
    let options = ProviderOptions {
        config: Some(directory.path().join("missing.toml")),
        ..Default::default()
    };
    assert_eq!(options.resolve().unwrap_err().code, "SOURCE_CONFIGURATION");
}

#[test]
fn cache_identity_binds_provider_endpoint_but_not_credential() {
    let (_directory, options) = configured("[historical]\n");
    let original = options.resolve().unwrap();
    assert_eq!(original.source_id(), "alchemy-mainnet-archive");
    let (_directory, mut options) = configured("[historical]\n");
    options.key = Some("different-synthetic-secret".into());
    assert_eq!(original.source_id(), options.resolve().unwrap().source_id());
    let (_directory, options) =
        configured("[historical.providers.alchemy]\nurl='https://example.invalid/v2/{key}'\n");
    assert_ne!(original.source_id(), options.resolve().unwrap().source_id());
}
