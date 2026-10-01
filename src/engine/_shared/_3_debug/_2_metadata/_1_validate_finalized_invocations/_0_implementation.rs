use super::*;

/// Bind independently observed RSP identities to the worker's complete debug
/// invocation list. Register-capture invocations may be truncated and cannot be
/// substituted for this list.
pub fn validate_finalized_invocations(
    captured: &serde_json::Value,
    live: &[InvocationMetadata],
    maximum: usize,
) -> Result<Vec<u16>, Error> {
    require(
        (1..=1024).contains(&maximum),
        "DEBUG_CONFIG",
        "invalid finalized invocation limit",
    )?;
    let captures = captured["captures"]
        .as_array()
        .ok_or_else(|| error("DEBUG_PROTOCOL", "finalized execution captures missing"))?;
    require(
        captures.len() <= 64,
        "DEBUG_LIMIT",
        "finalized execution count exceeds live session bound",
    )?;
    let mut ordinals = std::collections::BTreeSet::new();
    let mut finalized = BTreeMap::new();
    for execution in captures {
        let ordinal = execution["ordinal"]
            .as_u64()
            .ok_or_else(|| error("DEBUG_PROTOCOL", "execution ordinal is not u64"))?;
        require(
            ordinal < u64::from(u16::MAX) && ordinals.insert(ordinal as u16),
            "DEBUG_PROTOCOL",
            "duplicate or oversized execution ordinal",
        )?;
        let declared = execution["debug_invocations"]
            .as_array()
            .ok_or_else(|| error("DEBUG_PROTOCOL", "finalized live-debug metadata missing"))?;
        require(
            declared.len() <= maximum,
            "DEBUG_LIMIT",
            "finalized live invocation limit exceeded",
        )?;
        for value in declared {
            let metadata = InvocationMetadata::parse(value.as_str().ok_or_else(|| {
                error("DEBUG_PROTOCOL", "finalized debug metadata is not a string")
            })?)?;
            require(
                ordinal == u64::from(metadata.execution_index),
                "DEBUG_ROUTING",
                "finalized debug metadata belongs to another execution",
            )?;
            require(
                finalized
                    .insert(
                        (metadata.execution_index, metadata.identity.invocation_index),
                        metadata,
                    )
                    .is_none(),
                "DEBUG_ROUTING",
                "duplicate finalized live invocation",
            )?;
        }
    }
    require(
        finalized.len() == live.len(),
        "DEBUG_ROUTING",
        "live/finalized debugger invocation counts differ",
    )?;
    let mut observed = std::collections::BTreeSet::new();
    for metadata in live {
        let key = (metadata.execution_index, metadata.identity.invocation_index);
        require(
            observed.insert(key),
            "DEBUG_ROUTING",
            "duplicate observed live invocation",
        )?;
        let expected = finalized.get(&key).ok_or_else(|| {
            error(
                "DEBUG_ROUTING",
                "live invocation absent from finalized debugger metadata",
            )
        })?;
        require(
            expected.fields == metadata.fields,
            "DEBUG_ROUTING",
            "live runtime identity differs from finalized invocation/ELF",
        )?;
    }
    Ok(ordinals.into_iter().collect())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalizedInventoryPolicy {
    Complete,
    /// Only the pinned v4-2 full-capture role is qualified for this older output contract.
    V42OrdinalAndCapturedIntersection,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedInventory {
    pub execution_indices: Vec<u16>,
    pub assurance: &'static str,
    pub live_invocations: usize,
    pub finalized_invocations: Option<usize>,
    pub captured_intersections: usize,
}
