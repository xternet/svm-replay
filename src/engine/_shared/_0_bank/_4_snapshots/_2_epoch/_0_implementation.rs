use super::*;

pub(in super::super) fn epoch_binding(
    fixture: &Value,
    expected_genesis: Option<&str>,
) -> Check<()> {
    let runtime = field(fixture, "runtime")?;
    let Some(value) = object(runtime)?.get("epochStakes") else {
        return Ok(());
    };
    let (snapshot, source, proof) = bound_parts(value, EPOCH_FIELDS)?;
    require(
        field(&snapshot, "schema")? == "svm-current-bank-epoch-stakes/v1"
            && field(&snapshot, "complete")? == &json!(true),
        "complete snapshot schema required",
    )?;
    snapshot_context(&snapshot)?;
    digest(field(&snapshot, "clockDataSha256")?)?;
    let epoch = u64_string(field(&snapshot, "epoch")?)?;
    let total = u64_string(field(&snapshot, "totalStake")?)?;
    let mut sum = 0u64;
    let mut votes = BTreeSet::new();
    for vote in array(field(&snapshot, "voteStakes")?)? {
        shape(vote, &["voteAccount", "stake"])?;
        require(
            votes.insert(pubkey(field(vote, "voteAccount")?)?),
            "duplicate vote",
        )?;
        sum = sum
            .checked_add(u64_string(field(vote, "stake")?)?)
            .ok_or_else(|| "stake sum overflow".to_owned())?;
    }
    require(sum == total, "stake sum mismatch")?;
    source_and_proof(
        &snapshot,
        source,
        proof,
        &[
            "trusted-current-bank-epoch-snapshot/v1",
            "controlled-current-bank-epoch-snapshot/v1",
        ],
        "svm-bound-epoch-stakes/v1",
    )?;
    let target = field(fixture, "target")?;
    let clock = clock_data(
        field(fixture, "clock")?,
        slot(field(target, "targetSlot")?)?,
    )?;
    equal(
        field(&snapshot, "slot")?,
        field(target, "targetSlot")?,
        "Clock/Bank slot",
    )?;
    equal(
        field(&snapshot, "parentSlot")?,
        field(target, "parentSlot")?,
        "Clock/Bank parent",
    )?;
    require(
        string(field(&snapshot, "clockDataSha256")?)? == sha256(&clock),
        "Clock data hash mismatch",
    )?;
    let clock_epoch = u64::from_le_bytes(
        clock[16..24]
            .try_into()
            .map_err(|error| format!("Clock epoch: {error}"))?,
    );
    require(epoch == clock_epoch, "Clock epoch mismatch")?;
    if let Some(genesis) = expected_genesis {
        pubkey(&json!(genesis))?;
        equal(field(&snapshot, "genesisHash")?, &json!(genesis), "genesis")?;
    }
    let bank = field(runtime, "bankContext")?;
    runtime_matches(&snapshot, field(runtime, "binding")?)?;
    equal(
        field(&snapshot, "clockDataSha256")?,
        field(runtime, "clockDataHash")?,
        "runtime Clock data",
    )?;
    equal(
        field(&snapshot, "blockEvidenceSha256")?,
        field(bank, "blockSourceHash")?,
        "bank block evidence",
    )?;
    equal(
        field(&snapshot, "slot")?,
        field(bank, "targetSlot")?,
        "bank slot",
    )?;
    equal(
        field(&snapshot, "parentSlot")?,
        field(bank, "parentSlot")?,
        "bank parent",
    )?;
    lineage(&snapshot, bank)
}
