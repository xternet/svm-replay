use super::*;

#[test]
fn archived_legacy_v0_and_every_v1_configuration_round_trip_exact_bytes() {
    for versioned in [false, true] {
        let mut bytes = vec![1];
        bytes.extend([6; 64]);
        if versioned {
            bytes.push(128);
        }
        bytes.extend([1, 0, 1, 3]);
        for key in [1, 2, 3] {
            bytes.extend([key; 32]);
        }
        bytes.extend([4; 32]);
        bytes.extend([1, 2, 130, 1]);
        bytes.extend([1; 130]);
        bytes.extend([128, 1]);
        bytes.extend([7; 128]);
        if versioned {
            bytes.extend([1]);
            bytes.extend([8; 32]);
            bytes.extend([2, 0, 2, 1, 1]);
        }
        let archive = super::super::decode(&STANDARD.encode(&bytes)).unwrap();
        assert_eq!(STANDARD.decode(encode(&archive).unwrap()).unwrap(), bytes);
    }
    for mask in 0..32 {
        if matches!(mask & 3, 1 | 2) {
            continue;
        }
        let bytes = super::super::_0_decode::tests::wire(mask);
        let archive = super::super::decode(&STANDARD.encode(&bytes)).unwrap();
        assert_eq!(STANDARD.decode(encode(&archive).unwrap()).unwrap(), bytes);
    }
}

#[test]
fn archive_encoder_rejects_missing_ambiguous_and_out_of_range_fields() {
    let value =
        super::super::decode(&STANDARD.encode(super::super::_0_decode::tests::wire(31))).unwrap();
    for (pointer, replacement) in [
        ("/version", json!(2)),
        ("/transaction/signatures/0", json!("bad")),
        ("/transaction/message/recentBlockhash", Value::Null),
        ("/transaction/message/instructions/0/accounts/0", json!(256)),
        (
            "/transaction/message/transactionConfig/computeUnitLimit",
            json!(u64::MAX),
        ),
        (
            "/transaction/message/header/numRequiredSignatures",
            json!(0),
        ),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        assert!(encode(&invalid).is_err(), "{pointer}");
    }
    let mut missing = value;
    missing["transaction"]["message"]["transactionConfig"]
        .as_object_mut()
        .unwrap()
        .remove("heapSize");
    assert!(encode(&missing).is_err());
}
