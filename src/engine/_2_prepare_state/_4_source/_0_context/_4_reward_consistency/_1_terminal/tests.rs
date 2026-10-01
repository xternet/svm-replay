use super::*;

fn image(slot: u64) -> Value {
    let mut bytes = vec![0u8; 81];
    bytes[..8].copy_from_slice(&100u64.to_le_bytes());
    bytes[8..16].copy_from_slice(&3u64.to_le_bytes());
    bytes[64..72].copy_from_slice(&900u64.to_le_bytes());
    bytes[72..80].copy_from_slice(&899u64.to_le_bytes());
    bytes[80] = 1;
    json!({"pubkey":stake::EPOCH_REWARDS,"presence":"present","role":"sysvar",
        "sourceSlot":slot,"dataBase64":STANDARD.encode(bytes),"lamports":"2000000",
        "rentEpoch":u64::MAX.to_string(),"executable":false,
        "owner":"Sysvar1111111111111111111111111111111111111"})
}

fn context() -> TerminalContext<'static> {
    TerminalContext {
        slot: 220,
        write_slot: 200,
        write_height: 102,
        height: 110,
        write_epoch: 3,
        epoch: 3,
        executor: "litesvm-v0.14.0-pr402-agave-4.1.2",
        block_revenue_sharing: false,
        rent_minimum: 1454640,
    }
}

#[test]
fn terminal_write_preserves_rounding_gap_and_native_metadata() {
    let old = image(220);
    let recovered = recover(&old, &image(200), &context()).unwrap();
    let bytes = STANDARD
        .decode(recovered["dataBase64"].as_str().unwrap())
        .unwrap();
    assert_eq!(bytes[80], 0);
    assert_eq!(u64::from_le_bytes(bytes[72..80].try_into().unwrap()), 899);
    for field in ["sourceSlot", "lamports", "owner", "rentEpoch", "executable"] {
        assert_eq!(recovered[field], old[field]);
    }
    assert_eq!(
        STANDARD
            .decode(old["dataBase64"].as_str().unwrap())
            .unwrap()[80],
        1
    );
}

#[test]
fn revenue_sharing_uses_native_rent_reserve_not_observed_balance() {
    let mut ctx = context();
    ctx.executor = "litesvm-v0.16.0-agave-4.2.1";
    for active in [false, true] {
        ctx.block_revenue_sharing = active;
        let result = recover(&image(220), &image(200), &ctx).unwrap();
        assert_eq!(
            result["lamports"],
            if active { "1454640" } else { "2000000" }
        );
    }
}

#[test]
fn wrong_position_epoch_runtime_and_values_reject() {
    for mode in 0..10 {
        let mut ctx = context();
        let old = image(220);
        let mut write = image(200);
        match mode {
            0 => ctx.write_height = 101,
            1 => ctx.height = 101,
            2 => ctx.write_epoch = 2,
            3 => ctx.write_slot = 221,
            4 => ctx.executor = "unreviewed",
            5 => ctx.block_revenue_sharing = true,
            6 => write["lamports"] = json!("1999999"),
            7 => write["owner"] = json!("wrong"),
            8 => write["sourceSlot"] = json!(199),
            9 => ctx.rent_minimum = 2000001,
            _ => unreachable!(),
        }
        assert!(recover(&old, &write, &ctx).is_err(), "mode {mode}");
    }
}

#[test]
fn malformed_or_ambiguous_reward_state_is_not_corrected() {
    for mode in 0..6 {
        let mut old = image(220);
        let mut bytes = STANDARD
            .decode(old["dataBase64"].as_str().unwrap())
            .unwrap();
        match mode {
            0 => {
                bytes.pop();
            }
            1 => bytes[80] = 2,
            2 => bytes[80] = 0,
            3 => bytes[8..16].copy_from_slice(&1u64.to_le_bytes()),
            4 => bytes[..8].copy_from_slice(&u64::MAX.to_le_bytes()),
            5 => bytes[72..80].copy_from_slice(&901u64.to_le_bytes()),
            _ => unreachable!(),
        }
        old["dataBase64"] = json!(STANDARD.encode(bytes));
        let mut write = old.clone();
        write["sourceSlot"] = json!(200);
        assert!(recover(&old, &write, &context()).is_err(), "mode {mode}");
    }
}
