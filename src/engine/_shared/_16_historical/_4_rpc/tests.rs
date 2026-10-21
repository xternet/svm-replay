use super::*;

#[test]
fn nullable_sdk_options_use_protocol_defaults_without_ignoring_invalid_values() {
    let config = _2_accounts::config(
        Some(&json!({"commitment":null,"minContextSlot":null,"dataSlice":null,"encoding":null})),
        10,
        true,
    )
    .unwrap();
    let image = json!({"presence":"present","dataBase64":"AAE=","owner":"owner","executable":false,
        "lamports":"1","rentEpoch":"0"});
    let result = _2_accounts::image(&image, &config).unwrap();
    assert_eq!(result["data"][1], "base58");
    assert_eq!(
        bs58::decode(result["data"][0].as_str().unwrap())
            .into_vec()
            .unwrap(),
        [0, 1]
    );
    assert_eq!(
        _2_accounts::config(Some(&json!({"minContextSlot":11})), 10, false)
            .unwrap_err()
            .code,
        -32016
    );
    assert!(_2_accounts::config(Some(&json!({"commitment":"invented"})), 10, false).is_err());
    assert!(_2_accounts::config(Some(&json!({"minContextSlot":-1})), 10, false).is_err());
    assert!(_2_accounts::config(Some(&json!({"encoding":"base64"})), 10, false).is_err());
}

#[test]
fn rpc_envelopes_distinguish_invalid_requests_notifications_and_ids() {
    let request =
        _0_request::decode(&json!({"jsonrpc":"2.0","id":"case","method":"getSlot"})).unwrap();
    assert_eq!(request.id, Some(json!("case")));
    assert!(request.params.is_empty());
    assert!(
        _0_request::decode(&json!({"jsonrpc":"2.0","method":"getSlot"}))
            .unwrap()
            .id
            .is_none()
    );
    for value in [
        json!({}),
        json!([]),
        json!({"jsonrpc":"1.0","id":1,"method":"getSlot"}),
        json!({"jsonrpc":"2.0","id":{},"method":"getSlot"}),
        json!({"jsonrpc":"2.0","id":1,"method":""}),
    ] {
        assert_eq!(_0_request::decode(&value).unwrap_err().code, -32600);
    }
    assert_eq!(
        _0_request::decode(&json!({"jsonrpc":"2.0","id":3,"method":"getSlot","params":{}}))
            .unwrap_err()
            .code,
        -32602
    );
}

#[test]
fn rpc_account_images_preserve_u64s_and_apply_byte_slices_without_fake_absence() {
    let image = json!({"pubkey":"account","presence":"present","dataBase64":"AAECAwQ=","owner":"owner",
        "lamports":u64::MAX.to_string(),"rentEpoch":u64::MAX.to_string(),"executable":false});
    let config = json!({"encoding":"base64","dataSlice":{"offset":1,"length":3}});
    let value = _2_accounts::image(&image, &config).unwrap();
    assert_eq!(value["lamports"].as_u64(), Some(u64::MAX));
    assert_eq!(value["rentEpoch"].as_u64(), Some(u64::MAX));
    assert_eq!(value["space"], 5);
    assert_eq!(value["data"], json!(["AQID", "base64"]));
    assert!(_2_accounts::image(&image, &json!({"encoding":"jsonParsed"})).is_err());
    assert!(_2_accounts::image(&image, &json!({"dataSlice":{"offset":-1,"length":1}})).is_err());
    assert!(_2_accounts::image(&json!({}), &config).is_err());
}

#[test]
fn base64_zstd_accounts_are_valid_raw_block_zstd_frames() {
    for size in [0_usize, 5, 128 * 1024, 300_000] {
        let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let frame = _2_accounts::zstd_frame(&data);
        assert_eq!(&frame[..5], &[0x28, 0xb5, 0x2f, 0xfd, 0xe0]);
        assert_eq!(
            u64::from_le_bytes(frame[5..13].try_into().unwrap()),
            size as u64
        );
        let (mut offset, mut out) = (13, Vec::new());
        loop {
            let header =
                u32::from_le_bytes([frame[offset], frame[offset + 1], frame[offset + 2], 0]);
            offset += 3;
            assert_eq!((header >> 1) & 3, 0, "raw block");
            let length = (header >> 3) as usize;
            assert!(length <= 128 * 1024);
            out.extend_from_slice(&frame[offset..offset + length]);
            offset += length;
            if header & 1 == 1 {
                break;
            }
        }
        assert_eq!((offset, out), (frame.len(), data));
    }
    let image = json!({"presence":"present","dataBase64":"AAE=","owner":"o","executable":false,
        "lamports":"1","rentEpoch":"0"});
    let value = _2_accounts::image(&image, &json!({"encoding":"base64+zstd"})).unwrap();
    assert_eq!(value["data"][1], "base64+zstd");
    assert!(_2_accounts::image(&image, &json!({"encoding":"jsonParsed"})).is_err());
}

#[test]
fn advance_crosses_epochs_only_on_an_explicit_synthetic_opt_in_and_keeps_its_wire_form() {
    use _3_dispatch::advance_options;
    for (options, expected) in [
        (None, (None, false)),
        (Some(json!(null)), (None, false)),
        (Some(json!({"unixTimestamp":5})), (Some(5), false)),
        (Some(json!({"syntheticEpoch":false})), (None, false)),
        (
            Some(json!({"syntheticEpoch":true,"unixTimestamp":null})),
            (None, true),
        ),
    ] {
        assert_eq!(advance_options(options.as_ref()).unwrap(), expected);
    }
    for invalid in [
        json!({"syntheticEpoch":"true"}),
        json!({"syntheticEpoch":1}),
        json!({"epoch":958}),
        json!({"unixTimestamp":"5"}),
        json!([true]),
    ] {
        assert_eq!(advance_options(Some(&invalid)).unwrap_err().code, -32602);
    }
    // Ordinary advances reach the worker (and the journal) exactly as before.
    let ordinary = SessionCommand::Advance {
        slots: 2,
        unix_timestamp: None,
        synthetic_epoch: false,
    };
    assert_eq!(
        serde_json::to_value(&ordinary).unwrap(),
        json!({"method":"advance","params":{"slots":2,"unixTimestamp":null}})
    );
    let warp: SessionCommand = serde_json::from_value(
        json!({"method":"advance","params":{"slots":2,"unixTimestamp":null,"syntheticEpoch":true}}),
    )
    .unwrap();
    assert!(matches!(
        warp,
        SessionCommand::Advance {
            synthetic_epoch: true,
            ..
        }
    ));
}
