use super::*;

#[allow(clippy::too_many_arguments)]
pub fn resolve_requested_dependencies(
    encoded: &str,
    original: &Value,
    transactions: &[SemanticTransaction],
    target_index: u64,
    parent_slot: u64,
    history: &mut History<'_>,
    boundary: Option<&RequestedBoundaryEvidence>,
    overrides: &[RequestedAccountOverride],
) -> Result<RequestedResolution> {
    if parent_slot >= history.target_slot
        || boundary.is_some_and(|e| e.target_slot != history.target_slot)
    {
        return Err(Error::new(
            "SOURCE_CONTEXT_MISMATCH",
            "requested boundary differs from historical source target",
        ));
    }
    resolve_requested_dependencies_with_accounts(
        encoded,
        original,
        transactions,
        target_index,
        parent_slot,
        |keys, slot| history.accounts(keys, slot, &Roles::default()),
        boundary,
        overrides,
    )
}
