use super::*;

#[test]
fn prefixed_selectors_round_trip_without_coercion() {
    let signature = "1".repeat(64);
    for value in [
        format!("tx:{signature}"),
        "slot:0".into(),
        "slot:9007199254740991".into(),
    ] {
        assert_eq!(value.parse::<After>().unwrap().to_string(), value);
    }
    for invalid in [
        "",
        "block:42",
        "slot:+1",
        "slot:01",
        "slot: 1",
        "slot:1 ",
        "slot:9007199254740992",
        "tx:1",
    ] {
        assert_eq!(
            invalid.parse::<After>().unwrap_err().code,
            "INVALID_SELECTOR"
        );
    }
}

#[test]
fn bare_slots_and_signatures_resolve_like_prefixed_selectors() {
    let signature = bs58::encode([7_u8; 64]).into_string();
    for (bare, prefixed) in [
        ("0".to_owned(), "slot:0".to_owned()),
        ("413023261".to_owned(), "slot:413023261".to_owned()),
        (
            "9007199254740991".to_owned(),
            "slot:9007199254740991".to_owned(),
        ),
        (signature.clone(), format!("tx:{signature}")),
    ] {
        assert_eq!(
            bare.parse::<After>().unwrap(),
            prefixed.parse::<After>().unwrap()
        );
        assert_eq!(bare.parse::<After>().unwrap().to_string(), prefixed);
    }
}

#[test]
fn bare_selectors_reject_invalid_encodings_and_out_of_range_slots() {
    for invalid in [
        "",
        "01",
        "+1",
        "-1",
        " 42",
        "42 ",
        "1.5",
        "1e3",
        "9007199254740992",
        "18446744073709551616",
        "bad",
        "0OIl",
        &bs58::encode([7_u8; 32]).into_string(),
        &bs58::encode([7_u8; 65]).into_string(),
        &"1".repeat(64),
    ] {
        assert_eq!(
            invalid.parse::<After>().unwrap_err().code,
            "INVALID_SELECTOR",
            "{invalid}"
        );
    }
}
