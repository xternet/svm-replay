use super::*;

pub fn validate_finalized_inventory(
    captured: &serde_json::Value,
    live: &[InvocationMetadata],
    maximum: usize,
    policy: FinalizedInventoryPolicy,
) -> Result<FinalizedInventory, Error> {
    let captures = captured["captures"]
        .as_array()
        .ok_or_else(|| error("DEBUG_PROTOCOL", "finalized execution captures missing"))?;
    if policy == FinalizedInventoryPolicy::Complete
        || captures
            .iter()
            .any(|execution| execution.get("debug_invocations").is_some())
    {
        let execution_indices = validate_finalized_invocations(captured, live, maximum)?;
        return Ok(FinalizedInventory {
            execution_indices,
            assurance: "complete-finalized-debug-invocation-inventory-matches-live-rsp",
            live_invocations: live.len(),
            finalized_invocations: Some(live.len()),
            captured_intersections: 0,
        });
    }
    require(
        (1..=1024).contains(&maximum) && captures.len() <= 64,
        "DEBUG_LIMIT",
        "invalid invocation/execution bound",
    )?;
    let mut observed = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for metadata in live {
        let key = (metadata.execution_index, metadata.identity.invocation_index);
        require(
            observed.insert(key, metadata).is_none(),
            "DEBUG_ROUTING",
            "duplicate live invocation",
        )?;
        let count = counts.entry(metadata.execution_index).or_insert(0usize);
        *count += 1;
        require(
            *count <= maximum,
            "DEBUG_LIMIT",
            "live invocation bound exceeded",
        )?;
    }
    let mut ordinals = std::collections::BTreeSet::new();
    let mut intersections = 0;
    for execution in captures {
        let ordinal = execution["ordinal"]
            .as_u64()
            .ok_or_else(|| error("DEBUG_PROTOCOL", "execution ordinal is not u64"))?;
        require(
            ordinal < u64::from(u16::MAX) && ordinals.insert(ordinal as u16),
            "DEBUG_PROTOCOL",
            "duplicate or oversized execution ordinal",
        )?;
        require(
            execution["execution_mode"] == "interpreter-debug"
                && matches!(
                    execution["disposition"].as_str(),
                    Some("COMPLETE" | "TRUNCATED")
                ),
            "DEBUG_PROTOCOL",
            "finalized execution mode/disposition differs",
        )?;
        let recorded = execution["invocations"].as_array().ok_or_else(|| {
            error(
                "DEBUG_PROTOCOL",
                "bounded register invocation inventory missing",
            )
        })?;
        require(
            recorded.len() <= maximum,
            "DEBUG_LIMIT",
            "captured invocation bound exceeded",
        )?;
        let mut ids = std::collections::BTreeSet::new();
        for invocation in recorded {
            let id = invocation["invocation_index"]
                .as_u64()
                .ok_or_else(|| error("DEBUG_PROTOCOL", "captured invocation index missing"))?;
            require(
                id < u64::from(u16::MAX) && ids.insert(id),
                "DEBUG_PROTOCOL",
                "duplicate or oversized captured invocation",
            )?;
            let metadata = observed.get(&(ordinal as u16, id as u16)).ok_or_else(|| {
                error(
                    "DEBUG_ROUTING",
                    "captured SBPF invocation has no independently observed live RSP identity",
                )
            })?;
            let caller = invocation["caller_index"]
                .as_u64()
                .ok_or_else(|| error("DEBUG_PROTOCOL", "captured caller missing"))?;
            let expected_caller = match metadata.identity.parent_index {
                Some(index) => caller == u64::from(index),
                None => caller == u64::from(u16::MAX) || caller == u64::MAX,
            };
            require(
                expected_caller
                    && invocation["depth"].as_u64() == Some(u64::from(metadata.identity.depth))
                    && invocation["program_id"].as_str()
                        == Some(metadata.identity.program.as_str())
                    && invocation["elf_sha256"].as_str() == Some(metadata.elf_sha256.as_str()),
                "DEBUG_ROUTING",
                "captured invocation differs from independently observed caller/depth/program/ELF",
            )?;
            intersections += 1;
        }
    }
    require(
        live.iter()
            .all(|metadata| ordinals.contains(&metadata.execution_index)),
        "DEBUG_ROUTING",
        "live runtime execution absent from finalized output",
    )?;
    Ok(FinalizedInventory{execution_indices:ordinals.into_iter().collect(),assurance:"live-rsp-identities; finalized-execution-ordinals-and-captured-intersection-only; no-complete-finalized-invocation-inventory",live_invocations:live.len(),finalized_invocations:None,captured_intersections:intersections})
}
