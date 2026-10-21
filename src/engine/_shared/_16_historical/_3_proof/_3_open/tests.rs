use super::_1_capability::required_capability;
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

fn wire_commands(encoded: String) -> Vec<SessionCommand> {
    vec![
        SessionCommand::RpcSend {
            transaction_base64: encoded.clone(),
            skip_preflight: true,
        },
        SessionCommand::RpcSimulate {
            transaction_base64: encoded.clone(),
            sig_verify: true,
            replace_recent_blockhash: false,
        },
        SessionCommand::Send {
            transaction_base64: encoded.clone(),
            sig_verify: true,
        },
        SessionCommand::Simulate {
            transaction_base64: encoded.clone(),
            sig_verify: true,
        },
        SessionCommand::FeeForMessage {
            message_base64: encoded,
        },
    ]
}

#[test]
fn every_v1_execution_and_fee_route_requires_declared_native_support() {
    // Header-only classifier input, not an executable transaction fixture.
    // SIMD-0385 puts the v1 discriminator (129) before the message/signatures.
    for command in wire_commands(STANDARD.encode([129, 1])) {
        assert_eq!(
            required_capability(&command).unwrap(),
            Some(("native-transaction-v1/v1", "transaction format v1"))
        );
    }
}

#[test]
fn legacy_v0_and_non_wire_operations_keep_their_existing_requirements() {
    for prefix in [0, 1, 128] {
        for command in wire_commands(STANDARD.encode([prefix, 0])) {
            assert_eq!(required_capability(&command).unwrap(), None);
        }
    }
    assert_eq!(required_capability(&SessionCommand::State).unwrap(), None);
    assert_eq!(
        required_capability(&SessionCommand::OverrideAccounts { entries: vec![] }).unwrap(),
        Some(("native-account-overrides/v1", "session account overrides"))
    );
}

#[test]
fn malformed_wire_encoding_is_an_explicit_input_error() {
    for command in wire_commands("not base64!".to_owned()) {
        assert_eq!(
            required_capability(&command).unwrap_err().code,
            "INVALID_TRANSACTION"
        );
    }
}
