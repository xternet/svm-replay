use super::*;

/// Classify normalized exact parent rows, retaining Deploy-explained absence.
pub fn build_inspected_loader_parent_accounts(
    records: &[Value],
    parent_slot: u64,
    roles: &Roles,
    evidence: &[IncludedTransaction],
    target_index: u64,
) -> Result<Vec<Value>> {
    let mut deployed = BTreeSet::new();
    for entry in evidence {
        safe_integer(entry.index, "loader evidence index")?;
        check(
            entry.index <= target_index,
            "loader evidence outside target boundary",
        )?;
        for i in resolve(&entry.raw, entry.index)? {
            if i.program == LOADER && deploy(&i) {
                deployed.insert(i.accounts[1].clone());
                deployed.insert(i.accounts[2].clone());
            }
        }
    }
    records
        .iter()
        .map(|record| {
            let key = record["pubkey"]
                .as_str()
                .ok_or_else(|| invalid("loader parent pubkey missing"))?;
            let special = record["presence"] == "absent"
                && deployed.contains(key)
                && (roles.programs.contains(key) || roles.program_data.contains(key));
            if !special {
                return classify_account(record.clone(), key, parent_slot, roles);
            }
            let mut bounded = roles.clone();
            bounded.programs.remove(key);
            bounded.program_data.remove(key);
            let mut account = classify_account(record.clone(), key, parent_slot, &bounded)?;
            account["role"] = json!(if roles.program_data.contains(key) {
                "programdata"
            } else {
                "program"
            });
            Ok(account)
        })
        .collect()
}

/// Inspect the caller's complete original-control/requested union, including failed transactions.
pub fn assert_program_lifecycle_supported(
    transactions: &[SemanticTransaction],
    raw_transactions: Option<&[Value]>,
    context: Option<&HistoricalLoaderContext>,
) -> Result<Vec<u64>> {
    let lifecycle = |t: &&SemanticTransaction| {
        t.program_ids
            .iter()
            .any(|p| [LOADER, LOADER_V4, ALT].contains(&p.as_str()))
    };
    let indices = transactions
        .iter()
        .filter(lifecycle)
        .map(|t| t.index)
        .collect();
    for t in transactions
        .iter()
        .filter(|t| t.program_ids.iter().any(|p| p == LOADER || p == LOADER_V4))
    {
        let run = || -> Result<()> {
            let c = context.ok_or_else(|| invalid("exact loader parent context missing"))?;
            let raw = raw_transactions
                .and_then(|v| usize::try_from(t.index).ok().and_then(|i| v.get(i)))
                .ok_or_else(|| invalid("raw loader instruction evidence missing"))?;
            assert_inspected_loader_lifecycle(raw, t.index, c)
        };
        run().map_err(|e| {
            unsupported(
                "UNSUPPORTED_PROGRAM_LIFECYCLE",
                "PREFLIGHT",
                format!("transaction {}: {}", t.index, e.message),
            )
        })?;
    }
    for t in transactions
        .iter()
        .filter(|t| t.program_ids.iter().any(|p| p == ALT))
    {
        let reject = |message: String| {
            unsupported(
                "UNSUPPORTED_ALT_LIFECYCLE",
                "PREFLIGHT",
                format!("transaction {}: {message}", t.index),
            )
        };
        let raw = raw_transactions
            .and_then(|v| usize::try_from(t.index).ok().and_then(|i| v.get(i)))
            .ok_or_else(|| reject("raw instruction evidence missing".into()))?;
        let instructions: Vec<_> = resolve(raw, t.index)
            .map_err(|e| reject(format!("instruction evidence malformed: {}", e.message)))?
            .into_iter()
            .filter(|i| i.program == ALT)
            .collect();
        if instructions.is_empty() {
            return Err(reject(
                "invoked ALT program has no instruction evidence".into(),
            ));
        }
        for i in instructions {
            let valid = match tag(&i.data) {
                Some(0) => i.data.len() == 13 && i.accounts.len() >= 4,
                Some(1 | 3) => i.data.len() == 4 && i.accounts.len() >= 2,
                Some(2) => {
                    i.data.len() >= 12
                        && (i.data.len() - 12) % 32 == 0
                        && (i.data.len() - 12) as u64 / 32 == uint(&i.data, 4)?
                        && i.accounts.len() >= 2
                }
                Some(4) => i.data.len() == 4 && i.accounts.len() >= 3,
                _ => false,
            };
            if !valid {
                return Err(reject(format!(
                    "{} instruction {} is not an inspected ALT lifecycle instruction",
                    if i.inner { "inner" } else { "outer" },
                    i.index
                )));
            }
        }
    }
    Ok(indices)
}
