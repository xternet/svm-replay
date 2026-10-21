use super::*;

#[test]
fn transaction_options_are_strict_and_follow_nullable_sdk_defaults() {
    let options = _0_options::config(Some(&json!({"encoding":"base64","sigVerify":null,"replaceRecentBlockhash":null,"commitment":null})), 10, true).unwrap();
    assert!(!_0_options::flag(&options, "sigVerify", false).unwrap());
    assert!(!_0_options::flag(&options, "replaceRecentBlockhash", false).unwrap());
    for options in [
        json!({"unknown":true}),
        json!({"sigVerify":"yes"}),
        json!({"sigVerify":true,"replaceRecentBlockhash":true}),
        json!({"minContextSlot":11}),
    ] {
        assert!(
            _0_options::config(Some(&options), 10, true).is_err(),
            "{options}"
        );
    }
    assert!(_0_options::config(Some(&json!({"skipPreflight":0})), 10, false).is_err());
    // SDK retry counts are accepted: the fork executes on arrival and never rebroadcasts.
    assert!(_0_options::config(Some(&json!({"maxRetries":5})), 10, false).is_ok());
    assert!(_0_options::config(Some(&json!({"maxRetries":-1})), 10, false).is_err());
    let bytes = vec![0; 1232];
    assert_eq!(
        _0_options::wire(
            &json!(STANDARD.encode(&bytes)),
            &json!({"encoding":"base64"})
        )
        .unwrap(),
        STANDARD.encode(&bytes)
    );
    assert_eq!(
        _0_options::wire(&json!(bs58::encode(&bytes).into_string()), &json!({})).unwrap(),
        STANDARD.encode(&bytes)
    );
    for (wire, config) in [
        (
            json!(STANDARD.encode([0; 1233])),
            json!({"encoding":"base64"}),
        ),
        (json!("?"), json!({})),
        (json!(""), json!({"encoding":"hex"})),
    ] {
        assert!(_0_options::wire(&wire, &config).is_err());
    }
}
