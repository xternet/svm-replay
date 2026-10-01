use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use svm_replay_engine::_2_prepare_state::transaction::{assert_historical, decode};

fn wire() -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend([7; 64]);
    bytes.extend([1, 0, 1, 2]);
    bytes.extend([1; 32]);
    bytes.extend([2; 32]);
    bytes.extend([3; 32]);
    bytes.extend([1, 1, 1, 0, 2, 4, 5]);
    bytes
}

#[test]
fn exact_wire_binds_to_archive_and_rejects_modified_message() {
    let encoded = STANDARD.encode(wire());
    let decoded = decode(&encoded).unwrap();
    let mut archive = json!({"version":"legacy", "transaction":decoded["transaction"].clone()});
    assert_historical(&encoded, &archive).unwrap();
    archive["transaction"]["message"]["recentBlockhash"] =
        json!(bs58::encode([9; 32]).into_string());
    assert!(assert_historical(&encoded, &archive).is_err());
}

#[test]
fn malformed_shortvec_trailing_and_header_reject() {
    for bytes in [
        vec![0x81, 0],
        {
            let mut b = wire();
            b.push(0);
            b
        },
        {
            let mut b = wire();
            b[65] = 2;
            b
        },
    ] {
        assert!(decode(&STANDARD.encode(bytes)).is_err());
    }
}

#[test]
fn malformed_instruction_account_index_rejects() {
    let mut b = wire();
    let n = b.len();
    b[n - 4] = 10;
    assert!(decode(&STANDARD.encode(b)).is_err());
}
