use super::*;

pub(crate) const EPOCH_REWARDS: &str = "SysvarEpochRewards1111111111111111111111111";

pub(crate) fn supports_inactive_rewards(executor: &str) -> bool {
    matches!(
        executor,
        "litesvm-v0.6.1-agave-2.2.20"
            | "litesvm-v0.7.1-agave-2.3.9"
            | "litesvm-v0.8.2-agave-3.0.10"
            | "litesvm-v0.12.0-agave-3.1.11"
            | "litesvm-v0.13.1-agave-4.0.0"
            | "litesvm-v0.14.0-pr402-agave-4.1.2"
            | "litesvm-v0.16.0-agave-4.2.1"
    )
}

pub(in super::super) fn initialize_inactive(input: &Value, evidence: &Value) -> Result<Value> {
    let runtime = field(input, "runtime")?;
    // Reviewed Bank versions distribute rewards before transactions. Legacy
    // rent safety is checked below; later eras require a separate phase review.
    let executor = string(field(runtime, "executorSourceId")?)?;
    check(
        supports_inactive_rewards(executor),
        "inactive reward proof requires a reviewed Bank era",
    )?;
    // 2.2.20 is reviewed here only for completed reward intervals, not the
    // separate positive-reward reconstruction admitted by ERAS.
    let (version, commit) = if executor == "litesvm-v0.6.1-agave-2.2.20" {
        ("2.2.20", "dabc99a539c4b024a715cb247a4decf7e45d8659")
    } else {
        let (_, version, commit) = ERAS
            .iter()
            .find(|(name, _, _)| *name == executor)
            .ok_or_else(|| fail("inactive reward source identity missing"))?;
        (*version, *commit)
    };
    check(
        !array(field(runtime, "activeExecutionFeatureIds")?)?
            .iter()
            .any(|id| id == "a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"),
        "unreviewed Bank era",
    )?;
    let slot = integer(field(input, "slot")?)?;
    let parent = integer(field(input, "parentSlot")?)?;
    let block = field(field(input, "envelope")?, "result")?;
    check(
        parent < slot && field(block, "parentSlot")? == &json!(parent),
        "parent boundary differs",
    )?;
    let height = integer(field(block, "blockHeight")?)?;
    let data = phase_bytes(field(evidence, "account")?, EPOCH_REWARDS, slot, 81)?;
    let start = read_u64(&data, 0)?;
    let partitions = read_u64(&data, 8)?;
    let end = start
        .checked_add(partitions)
        .ok_or_else(|| fail("reward interval overflow"))?;
    // active=false alone is insufficient: the final distribution block clears it.
    check(
        data[80] == 0 && start > 0 && partitions > 0 && height >= end,
        "stake reward distribution may occur in this block",
    )?;
    let hashes = array(field(evidence, "evidenceHashes")?)?;
    check(!hashes.is_empty(), "EpochRewards provenance missing")?;
    for value in hashes {
        digest(string(value)?)?;
    }
    let accounts = array(field(input, "accounts")?)?;
    let mut proofs = Vec::new();
    for account in accounts
        .iter()
        .filter(|a| a.get("owner") == Some(&json!(STAKE)))
    {
        check(
            account.get("presence") == Some(&json!("present"))
                && account.get("sourceSlot") == Some(&json!(parent))
                && account.get("executable") == Some(&json!(false)),
            "parent stake boundary differs",
        )?;
        if matches!(version, "2.2.20" | "2.3.9") {
            check(
                account.get("rentEpoch") == Some(&json!(u64::MAX.to_string())),
                "legacy inactive stake requires the exempt rent marker",
            )?;
        }
        let bytes = bytes(field(account, "dataBase64")?)?;
        check(
            bytes.len() == 200
                && [1u32.to_le_bytes(), 2u32.to_le_bytes()]
                    .iter()
                    .any(|tag| bytes.starts_with(tag)),
            "unreviewed stake account layout",
        )?;
        proofs.push(json!({"schema":"svm-simulate-inactive-stake-initialization/v1",
            "phaseSourceCommit":commit,"executorSourceId":executor,
            "slot":slot,"parentSlot":parent,"blockHeight":height,
            "blockSourceSha256":field(input,"blockSourceHash")?,
            "pubkey":field(account,"pubkey")?,"initializedAccountSha256":hash(canonical_json(account)),
            "epochRewardsDataSha256":hash(&data),"evidenceHashes":hashes,
            "distributionStartingBlockHeight":start,"distributionEndExclusive":end}));
    }
    Ok(json!({"accounts":accounts,"proofs":proofs}))
}
