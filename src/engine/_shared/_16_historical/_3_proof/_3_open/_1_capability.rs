use super::*;

pub(super) const TX_V1: &str = "native-transaction-v1/v1";

pub(super) fn required_capability(
    command: &SessionCommand,
) -> Result<Option<(&'static str, &'static str)>, Error> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let encoded = match command {
        SessionCommand::RpcSend {
            transaction_base64, ..
        }
        | SessionCommand::RpcSimulate {
            transaction_base64, ..
        }
        | SessionCommand::Send {
            transaction_base64, ..
        }
        | SessionCommand::Simulate {
            transaction_base64, ..
        } => Some(transaction_base64),
        SessionCommand::FeeForMessage { message_base64 } => Some(message_base64),
        _ => None,
    };
    if let Some(encoded) = encoded {
        let wire = STANDARD.decode(encoded).map_err(|error| {
            Error::new(
                "INVALID_TRANSACTION",
                format!("invalid base64 wire input: {error}"),
            )
        })?;
        // SIMD-0385 puts the v1 discriminator before the message/signatures.
        // Legacy/v0 transactions still start with their shortvec signature count.
        if wire.first() == Some(&129) {
            return Ok(Some((TX_V1, "transaction format v1")));
        }
    }
    Ok(match command {
        SessionCommand::OverrideAccounts { .. } => {
            Some(("native-account-overrides/v1", "session account overrides"))
        }
        SessionCommand::Advance {
            synthetic_epoch: true,
            ..
        } => Some(("native-synthetic-epoch/v1", "synthetic epoch warp")),
        SessionCommand::Advance { .. } => Some(("native-slot-advance/v1", "slot advance")),
        SessionCommand::BlockhashValid { .. } | SessionCommand::ProveBlockhashQueue { .. } => {
            Some((AGES, "archived blockhash ages"))
        }
        _ => None,
    })
}
