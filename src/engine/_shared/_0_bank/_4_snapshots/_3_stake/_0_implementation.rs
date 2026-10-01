use super::*;

pub(in super::super) fn stake_account(account: &Value, source_slot: &Value) -> Check<()> {
    shape(
        account,
        &[
            "pubkey",
            "sourceSlot",
            "role",
            "presence",
            "owner",
            "executable",
            "lamports",
            "rentEpoch",
            "dataBase64",
        ],
    )?;
    pubkey(field(account, "pubkey")?)?;
    require(
        field(account, "sourceSlot")? == source_slot
            && field(account, "role")? == "application"
            && field(account, "presence")? == "present"
            && field(account, "owner")? == STAKE
            && field(account, "executable")? == &json!(false),
        "stake image slot/owner/presence/role mismatch",
    )?;
    u64_string(field(account, "lamports")?)?;
    u64_string(field(account, "rentEpoch")?)?;
    bytes(field(account, "dataBase64")?)?;
    Ok(())
}

pub(in super::super) fn initialized_snapshot(value: &Value) -> Check<Value> {
    let (snapshot, source, proof) = bound_parts(value, INITIALIZED_FIELDS)?;
    require(
        field(&snapshot, "schema")? == "svm-bank-initialized-stakes/v1"
            && field(&snapshot, "phase")? == "post-bank-initialization/pre-transaction",
        "snapshot phase/schema mismatch",
    )?;
    snapshot_context(&snapshot)?;
    let mut seen = BTreeSet::new();
    for row in array(field(&snapshot, "accounts")?)? {
        shape(row, &["parent", "initialized"])?;
        let parent = field(row, "parent")?;
        let initialized = field(row, "initialized")?;
        stake_account(parent, field(&snapshot, "parentSlot")?)?;
        stake_account(initialized, field(&snapshot, "slot")?)?;
        equal(
            field(parent, "pubkey")?,
            field(initialized, "pubkey")?,
            "stake pair identity",
        )?;
        require(
            seen.insert(string(field(parent, "pubkey")?)?),
            "duplicate stake pair",
        )?;
    }
    source_and_proof(
        &snapshot,
        source,
        proof,
        &[
            "trusted-bank-initialization/v1",
            "controlled-bank-initialization/v1",
        ],
        "svm-bound-initialized-stakes/v1",
    )?;
    Ok(snapshot)
}

pub(in super::super) fn initialized_binding(
    fixture: &Value,
    expected_genesis: Option<&str>,
) -> Check<()> {
    let runtime = field(fixture, "runtime")?;
    let Some(value) = object(runtime)?.get("initializedStakeSnapshot") else {
        return Ok(());
    };
    let snapshot = initialized_snapshot(value)?;
    if let Some(genesis) = expected_genesis {
        pubkey(&json!(genesis))?;
        equal(field(&snapshot, "genesisHash")?, &json!(genesis), "genesis")?;
    }
    if let Some(epoch) = runtime.get("epochStakes") {
        equal(
            field(epoch, "genesisHash")?,
            field(&snapshot, "genesisHash")?,
            "mixed Bank input genesis",
        )?;
    }
    let target = field(fixture, "target")?;
    let bank = field(runtime, "bankContext")?;
    equal(
        field(&snapshot, "slot")?,
        field(target, "targetSlot")?,
        "target slot",
    )?;
    equal(
        field(&snapshot, "parentSlot")?,
        field(target, "parentSlot")?,
        "target parent",
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
    equal(
        field(&snapshot, "blockEvidenceSha256")?,
        field(bank, "blockSourceHash")?,
        "bank block evidence",
    )?;
    runtime_matches(&snapshot, field(runtime, "binding")?)?;
    lineage(&snapshot, bank)?;
    required_stake_set(field(fixture, "accounts")?, &snapshot, "initialized")
}

pub(in super::super) fn required_stake_set(
    accounts: &Value,
    snapshot: &Value,
    phase: &str,
) -> Check<()> {
    let mut stakes = BTreeMap::new();
    for account in array(accounts)? {
        if account.get("presence") == Some(&json!("present"))
            && account.get("owner") == Some(&json!(STAKE))
        {
            require(
                stakes
                    .insert(string(field(account, "pubkey")?)?, account)
                    .is_none(),
                "duplicate required stake",
            )?;
        }
    }
    let rows = array(field(snapshot, "accounts")?)?;
    require(
        stakes.len() == rows.len(),
        "complete required stake set mismatch",
    )?;
    for row in rows {
        let initialized = field(row, phase)?;
        let key = string(field(initialized, "pubkey")?)?;
        let observed = stakes
            .get(key)
            .ok_or_else(|| format!("{phase} image missing: {key}"))?;
        require(
            canonical_json(observed) == canonical_json(initialized),
            &format!("{phase} image mismatch: {key}"),
        )?;
    }
    Ok(())
}
