use super::*;
use dependencies::SemanticTransaction;
use overrides::RequestedAccountOverride;
use requested::RequestedResolution;
use svm_replay_protocol::{StateOverride, WhatIfMode};

/// The wires that run at the target position: the what-if bundle (or the target itself when
/// only state overrides were given), or a 0.1.0 replacement / ALT-override re-run.
pub(in super::super::super) fn requested_transactions(
    input: &HistoricalRequest,
    a: &Analysis,
    original_wire: &str,
    overridden_lookup: bool,
    changes: &[RequestedAccountOverride],
    history: &mut History<'_>,
) -> Result<Vec<RequestedResolution>, Error> {
    let wires: Vec<&str> = match (&input.what_if, &input.replacement_transaction_base64) {
        (Some(what_if), _) => match &what_if.transactions_base64 {
            Some(list) => list.iter().map(String::as_str).collect(),
            None => vec![original_wire],
        },
        (None, Some(replacement)) => vec![replacement.as_str()],
        (None, None) if overridden_lookup => vec![original_wire],
        (None, None) => Vec::new(),
    };
    wires
        .into_iter()
        .map(|wire| {
            requested::resolve_requested_dependencies(
                wire,
                &a.raw[a.index as usize],
                &a.transactions,
                a.index,
                a.parent,
                history,
                Some(&requested::RequestedBoundaryEvidence {
                    target_slot: a.slot,
                    raw_transactions: a.raw.clone(),
                }),
                changes,
            )
        })
        .collect()
}

/// One summary for all requested transactions; None when there are none.
pub(in super::super::super) fn merged_summary(
    requested: &[RequestedResolution],
) -> Result<Option<SemanticTransaction>, Error> {
    let mut summaries = requested.iter().map(|value| &value.summary);
    let Some(first) = summaries.next() else {
        return Ok(None);
    };
    let mut merged = first.clone();
    for next in summaries {
        merged = dependencies::include_requested_dependencies(&merged, next)?;
    }
    Ok(Some(merged))
}

/// The target as the closure sees it: the original plus every requested account. Accounts a
/// state override replaces whole (or deletes) need no archive image, unless the original uses
/// them; accounts it patches do (their current fields are kept).
pub(in super::super::super) fn dependency_target(
    input: &HistoricalRequest,
    original: &SemanticTransaction,
    merged: Option<&SemanticTransaction>,
) -> Result<SemanticTransaction, Error> {
    let mut target = match merged {
        Some(summary) => dependencies::include_requested_dependencies(original, summary)?,
        None if input.requested_account_overrides.is_some() => {
            dependencies::include_requested_dependencies(original, original)?
        }
        None => original.clone(),
    };
    if let Some(what_if) = &input.what_if {
        // The engine resolves lookup tables before the target; the worker after it.
        let changed_table = merged.and_then(|summary| {
            summary
                .address_table_accounts
                .iter()
                .find(|table| original.writable_accounts.contains(table))
        });
        if let (WhatIfMode::After, Some(table)) = (what_if.mode, changed_table) {
            return Err(Error::new(
                "UNSUPPORTED_WHAT_IF",
                format!("{table}: the target changes this lookup table, so a transaction after it cannot use it yet"),
            ));
        }
        let used = |key: &String| {
            original.declared_accounts.contains(key)
                || original.semantic_dependency_accounts.contains(key)
        };
        let (replaced, patched): (Vec<_>, Vec<_>) = what_if
            .state_overrides
            .iter()
            .partition(|entry| replaces(entry));
        let skip: BTreeSet<_> = replaced
            .iter()
            .map(|entry| &entry.pubkey)
            .filter(|key| !used(key))
            .collect();
        for list in [
            &mut target.declared_accounts,
            &mut target.instruction_accounts,
            &mut target.writable_accounts,
            &mut target.application_writable_accounts,
            &mut target.address_table_accounts,
            &mut target.program_ids,
            &mut target.program_data_accounts,
            &mut target.semantic_dependency_accounts,
        ] {
            list.retain(|key| !skip.contains(key));
        }
        let keys = patched.iter().map(|entry| entry.pubkey.clone());
        for list in [
            &mut target.declared_accounts,
            &mut target.semantic_dependency_accounts,
        ] {
            *list = unique(list.iter().cloned().chain(keys.clone()));
        }
    }
    if !input.extra_accounts.is_empty() {
        for list in [
            &mut target.declared_accounts,
            &mut target.semantic_dependency_accounts,
        ] {
            *list = unique(
                list.iter()
                    .cloned()
                    .chain(input.extra_accounts.iter().cloned()),
            );
        }
    }
    Ok(target)
}

/// Programs the overrides place; their code is inspected like any program the bundle calls.
pub(in super::super::super) fn override_programs(input: &HistoricalRequest) -> Vec<String> {
    input
        .what_if
        .iter()
        .flat_map(|what_if| &what_if.state_overrides)
        .filter(|entry| entry.executable == Some(true))
        .map(|entry| entry.pubkey.clone())
        .collect()
}

/// With `owner` (or `lamports: "0"`) an override defines the whole account.
fn replaces(entry: &StateOverride) -> bool {
    entry.owner.is_some() || entry.lamports.as_deref() == Some("0")
}
