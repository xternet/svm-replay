use super::*;

pub(in super::super::super) fn validate(
    accounts: &[Value],
    executor: &str,
    slot: u64,
    block: &Value,
) -> Result<(), Error> {
    // Agave 4.1.2 and 4.2.1 partitioned_epoch_rewards/distribution.rs clear
    // active by the last partition. This check does not reconstruct omitted
    // distribution updates or certify the remaining fields of the image.
    if !matches!(
        executor,
        "litesvm-v0.14.0-pr402-agave-4.1.2" | "litesvm-v0.16.0-agave-4.2.1"
    ) {
        return Ok(());
    }
    for account in accounts
        .iter()
        .filter(|a| a["pubkey"] == stake::EPOCH_REWARDS)
    {
        let encoded = account["dataBase64"]
            .as_str()
            .ok_or_else(|| invalid("EpochRewards encoding"))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|e| invalid(format!("EpochRewards:{e}")))?;
        if bytes.len() != 81 || bytes[80] > 1 {
            return Err(invalid("EpochRewards layout"));
        }
        let read = |offset| -> Result<u64, Error> {
            Ok(u64::from_le_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .map_err(|_| invalid("EpochRewards integer"))?,
            ))
        };
        let start = read(0)?;
        let partitions = read(8)?;
        let end = start
            .checked_add(partitions)
            .ok_or_else(|| invalid("EpochRewards interval overflow"))?;
        let height = block["blockHeight"]
            .as_u64()
            .ok_or_else(|| invalid("EpochRewards block height unavailable"))?;
        if bytes[80] == 1 && start > 0 && partitions > 0 && height.saturating_add(1) >= end {
            return Err(Error::new("UNSUPPORTED_BANK_INPUT",
                "historical EpochRewards remains active after its distribution window; exact corrected image required")
                .with_details(json!({"reason":"inconsistent-epoch-rewards","pubkey":stake::EPOCH_REWARDS,
                    "slot":slot,"blockHeight":height,"distributionStartingBlockHeight":start,
                    "distributionEndExclusive":end,"accountSha256":Digest::of(canonical_json(account)),
                    "executorSourceId":executor})));
        }
    }
    Ok(())
}
