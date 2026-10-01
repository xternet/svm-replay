use super::*;

/// Adds request-only reads to prefix selection while retaining original control identity.
pub fn include_requested_dependencies(
    original: &SemanticTransaction,
    requested: &SemanticTransaction,
) -> Result<SemanticTransaction> {
    if original.index != requested.index {
        return Err(invalid("requested boundary index mismatch"));
    }
    let mut combined = original.clone();
    combined.declared_accounts = unique(
        original
            .declared_accounts
            .iter()
            .chain(&requested.declared_accounts)
            .cloned(),
    );
    combined.instruction_accounts = unique(
        original
            .instruction_accounts
            .iter()
            .chain(&requested.instruction_accounts)
            .cloned(),
    );
    combined.writable_accounts = unique(
        original
            .writable_accounts
            .iter()
            .chain(&requested.writable_accounts)
            .cloned(),
    );
    combined.application_writable_accounts = unique(
        original
            .application_writable_accounts
            .iter()
            .chain(&requested.application_writable_accounts)
            .cloned(),
    );
    combined.address_table_accounts = unique(
        original
            .address_table_accounts
            .iter()
            .chain(&requested.address_table_accounts)
            .cloned(),
    );
    combined.program_ids = unique(
        original
            .program_ids
            .iter()
            .chain(&requested.program_ids)
            .cloned(),
    );
    combined.semantic_dependency_accounts = unique(
        original
            .semantic_dependency_accounts
            .iter()
            .chain(&requested.declared_accounts)
            .chain(&requested.address_table_accounts)
            .cloned(),
    );
    combined.excluded_dependency_accounts = unique(
        original
            .excluded_dependency_accounts
            .iter()
            .chain(&requested.excluded_dependency_accounts)
            .cloned(),
    );
    Ok(combined)
}
