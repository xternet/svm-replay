use super::*;

fn account(key: &str, data: &[u8]) -> Value {
    json!({"pubkey":key,"sourceSlot":20,"role":"sysvar","presence":"present",
        "owner":"Sysvar1111111111111111111111111111111111111","executable":false,
        "lamports":"1","rentEpoch":"0","dataBase64":STANDARD.encode(data)})
}

fn rent(rate: u64, threshold: f64, burn: u8) -> Value {
    let mut data = rate.to_le_bytes().to_vec();
    data.extend(threshold.to_le_bytes());
    data.push(burn);
    account(RENT, &data)
}

#[test]
fn reserve_is_computed_from_historical_values() {
    for rate in [1u64, 71, 3167, 3480] {
        for threshold in [1.0, 1.5, 2.0] {
            let expected = ((128 + 81) as f64 * rate as f64 * threshold) as u64;
            assert_eq!(
                rent_minimum(&rent(rate, threshold, 50), 20).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn malformed_rent_and_wrong_identity_reject() {
    for image in [
        rent(0, 2.0, 50),
        rent(u64::MAX, 2.0, 50),
        rent(1, f64::NAN, 50),
        rent(1, f64::INFINITY, 50),
        rent(1, -1.0, 50),
        rent(1, 2.0, 101),
        account(RENT, &[0; 16]),
        account(CLOCK, &[0; 17]),
    ] {
        assert!(rent_minimum(&image, 20).is_err());
    }
    let valid = rent(3480, 2.0, 50);
    assert!(rent_minimum(&valid, 21).is_err());
    for (key, value) in [
        ("owner", json!("wrong")),
        ("executable", json!(true)),
        ("presence", json!("absent")),
        ("role", json!("ordinary")),
    ] {
        let mut changed = valid.clone();
        changed[key] = value;
        assert!(rent_minimum(&changed, 20).is_err());
    }
}

#[test]
fn epoch_requires_both_embedded_and_observed_slot() {
    let mut bytes = [0u8; 40];
    bytes[..8].copy_from_slice(&20u64.to_le_bytes());
    bytes[16..24].copy_from_slice(&7u64.to_le_bytes());
    assert_eq!(epoch(&account(CLOCK, &bytes), 20).unwrap(), 7);
    assert!(epoch(&account(CLOCK, &bytes), 21).is_err());
    bytes[..8].copy_from_slice(&19u64.to_le_bytes());
    assert!(epoch(&account(CLOCK, &bytes), 20).is_err());
    assert!(epoch(&account(CLOCK, &bytes[..24]), 20).is_err());
}
