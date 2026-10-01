use super::*;

pub fn requested_cannot_invoke_alt(requested: &SemanticTransaction) -> bool {
    !requested.declared_accounts.iter().any(|key| key == ALT)
}

pub fn assert_requested_parent_alt_still_usable(
    table: &str,
    writers: &[SemanticTransaction],
    parent_slot: u64,
    evidence: &RequestedBoundaryEvidence,
) -> Result<bool> {
    safe_integer(parent_slot, "parent slot")?;
    safe_integer(evidence.target_slot, "target slot")?;
    if evidence.target_slot <= parent_slot {
        return Err(invalid(
            "replacement ALT boundary must follow its actual parent",
        ));
    }
    let mut closed = false;
    let mut created = false;
    for writer in writers {
        let raw = evidence
            .raw_transactions
            .get(
                usize::try_from(writer.index).map_err(|_| {
                    invalid(format!("{table}: ALT writer evidence identity mismatch"))
                })?,
            )
            .ok_or_else(|| invalid(format!("{table}: ALT writer evidence identity mismatch")))?;
        if summarize_semantic_transaction(raw, writer.index)? != *writer {
            return Err(invalid(format!(
                "{table}: ALT writer evidence identity mismatch"
            )));
        }
        // Even failed writers require recorded CPI metadata before their rollback is trusted.
        let resolved = recorded_instructions(raw, writer.succeeded)?;
        if !writer.succeeded {
            continue;
        }
        for instruction in resolved {
            if instruction.program != ALT
                || instruction.accounts.first().is_none_or(|key| key != table)
            {
                continue;
            }
            let data = &instruction.data;
            let tag = if data.len() >= 4 {
                Some(u32_at(data, 0)?)
            } else {
                None
            };
            let valid = match tag {
                Some(0) => data.len() == 13 && instruction.accounts.len() >= 4,
                Some(1) | Some(3) => data.len() == 4 && instruction.accounts.len() >= 2,
                Some(2) => {
                    data.len() >= 12
                        && instruction.accounts.len() >= 2
                        && u128::from(data.len() as u64) == 12 + u128::from(u64_at(data, 4)?) * 32
                }
                Some(4) => data.len() == 4 && instruction.accounts.len() >= 3,
                _ => false,
            };
            if !valid {
                return Err(unsupported(format!(
                    "{table}: unrecognized committed ALT instruction at {}",
                    writer.index
                )));
            }
            if tag == Some(4) {
                closed = true;
            }
            if tag == Some(0) && u64_at(data, 4)? <= parent_slot {
                created = true;
            }
            if closed && tag == Some(0) {
                return Err(invalid(format!(
                    "INVALID_REQUESTED_LOOKUP_RECREATED:{table}"
                )));
            }
        }
    }
    if closed {
        return Err(invalid(format!("INVALID_REQUESTED_LOOKUP_CLOSED:{table}")));
    }
    Ok(created)
}
