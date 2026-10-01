use super::*;

pub(in super::super) fn validate_identity(identity: &Value) -> Result<()> {
    json_value(identity)?;
    object(
        identity,
        &[
            "id",
            "version",
            "genesisHash",
            "kind",
            "coverage",
            "capabilities",
        ],
        "INVALID_SOURCE",
        "source identity",
    )?;
    public_id(&identity["id"], "INVALID_SOURCE", "public source id")?;
    public_id(&identity["version"], "INVALID_SOURCE", "source version")?;
    address(
        &identity["genesisHash"],
        32,
        "INVALID_SOURCE",
        "source genesis",
    )?;
    require(
        identity["kind"] == "captured-history"
            || identity["kind"] == "supplied-bank"
            || identity["kind"] == "alchemy-historical",
        "INVALID_SOURCE",
        "source kind",
    )?;
    object(
        &identity["coverage"],
        &["firstSlot", "lastSlot", "completeness"],
        "INVALID_SOURCE",
        "coverage",
    )?;
    let first = slot(
        &identity["coverage"]["firstSlot"],
        "INVALID_SOURCE",
        "first slot",
    )?;
    let last = slot(
        &identity["coverage"]["lastSlot"],
        "INVALID_SOURCE",
        "last slot",
    )?;
    require(
        first <= last && identity["coverage"]["completeness"] == "partial",
        "INVALID_SOURCE",
        "observed coverage",
    )?;
    let capabilities = identity["capabilities"]
        .as_array()
        .ok_or_else(|| SourceError::new("INVALID_SOURCE", "source capabilities"))?;
    let mut seen = BTreeSet::new();
    require(
        !capabilities.is_empty(),
        "INVALID_SOURCE",
        "source capabilities",
    )?;
    for capability in capabilities {
        let name = text(capability, "INVALID_SOURCE", "source capabilities")?;
        require(
            seen.insert(name)
                && if identity["kind"] == "supplied-bank" {
                    name == "bank-input"
                } else {
                    ["block", "account", "transaction"].contains(&name)
                },
            "INVALID_SOURCE",
            "source capabilities",
        )?;
    }
    Ok(())
}

pub(in super::super) fn covers(identity: &Value, query: &Value) -> Result<bool> {
    require(
        query["genesisHash"] == identity["genesisHash"],
        "SOURCE_CONTEXT_MISMATCH",
        "source network mismatch",
    )?;
    let capabilities = identity["capabilities"]
        .as_array()
        .ok_or_else(|| SourceError::new("INVALID_SOURCE", "source capabilities"))?;
    let current = slot(&query["slot"], "INVALID_QUERY", "slot")?;
    Ok(capabilities.contains(&query["kind"])
        && current
            >= slot(
                &identity["coverage"]["firstSlot"],
                "INVALID_SOURCE",
                "first slot",
            )?
        && current
            <= slot(
                &identity["coverage"]["lastSlot"],
                "INVALID_SOURCE",
                "last slot",
            )?)
}
