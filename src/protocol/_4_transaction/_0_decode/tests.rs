use super::*;

fn wire(mask: u32) -> Vec<u8> {
    let mut bytes = vec![129, 1, 0, 1];
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes.extend_from_slice(&[4; 32]);
    bytes.extend_from_slice(&[2, 3]);
    for value in [1, 2, 3] {
        bytes.extend_from_slice(&[value; 32]);
    }
    for (bits, value, width) in [
        (3, 731_u64, 8),
        (4, 10_000, 4),
        (8, 1_000_000, 4),
        (16, 65_536, 4),
    ] {
        if mask & bits == bits {
            bytes.extend_from_slice(&value.to_le_bytes()[..width]);
        }
    }
    // Two headers, followed by two separate payloads.
    bytes.extend_from_slice(&[2, 2, 2, 0, 2, 1, 1, 0, 0, 1, 7, 9, 1, 5]);
    bytes.extend_from_slice(&[6; 64]);
    bytes
}

#[test]
fn v1_wire_preserves_all_optional_configuration_fields() {
    for mask in 0..32_u32 {
        let result = decode(&STANDARD.encode(wire(mask)));
        if mask & 3 == 1 || mask & 3 == 2 {
            assert!(result.is_err(), "partial priority fee mask {mask}");
            continue;
        }
        let value = result.unwrap();
        assert_eq!(value["version"], 1);
        let message = &value["transaction"]["message"];
        assert_eq!(
            message["accountKeys"][1],
            bs58::encode([2; 32]).into_string()
        );
        assert_eq!(message["instructions"][0]["accounts"], json!([0, 1]));
        assert_eq!(
            message["instructions"][0]["data"],
            bs58::encode([7, 9]).into_string()
        );
        assert_eq!(
            message["instructions"][1]["data"],
            bs58::encode([5]).into_string()
        );
        for (bits, field, expected) in [
            (3, "priorityFee", 731),
            (4, "computeUnitLimit", 10_000),
            (8, "loadedAccountsDataSizeLimit", 1_000_000),
            (16, "heapSize", 65_536),
        ] {
            assert_eq!(
                message["transactionConfig"][field],
                if mask & bits == bits {
                    json!(expected)
                } else {
                    Value::Null
                }
            );
        }
        let archive = json!({"version": 1, "transaction": value["transaction"]});
        assert_historical(&STANDARD.encode(wire(mask)), &archive).unwrap();
    }
}

#[test]
fn v1_rejects_truncation_trailing_bad_indices_and_changed_archive_config() {
    let bytes = wire(31);
    for end in 0..bytes.len() {
        assert!(
            decode(&STANDARD.encode(&bytes[..end])).is_err(),
            "truncation {end}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&STANDARD.encode(trailing)).is_err());
    for (offset, replacement) in [
        (4, 63),
        (1, 13),
        (40, 65),
        (41, 65),
        (158, 0),
        (158, 3),
        (166, 3),
    ] {
        let mut invalid = bytes.clone();
        invalid[offset] = replacement;
        assert!(
            decode(&STANDARD.encode(invalid)).is_err(),
            "offset {offset}"
        );
    }
    let value = decode(&STANDARD.encode(&bytes)).unwrap();
    let mut archive = json!({"version": 1, "transaction": value["transaction"]});
    archive["transaction"]["message"]["transactionConfig"]["priorityFee"] = json!(732);
    assert!(assert_historical(&STANDARD.encode(&bytes), &archive).is_err());
    archive["transaction"]["message"]
        .as_object_mut()
        .unwrap()
        .remove("transactionConfig");
    assert!(assert_historical(&STANDARD.encode(bytes), &archive).is_err());
}
