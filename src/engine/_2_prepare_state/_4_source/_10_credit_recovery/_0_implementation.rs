use super::*;

pub(in super::super) fn recover(
    input: &HistoricalRequest,
    a: &Analysis,
    history: &mut History<'_>,
    transactions: &[dependencies::SemanticTransaction],
    initial: dependencies::SemanticClosureResult,
) -> Result<CreditPlan, Error> {
    // The same reviewed-runtime gate applies to acquisition and prepared proofs.
    if !input.runtime_binding["executor"]["id"]
        .as_str()
        .is_some_and(credits::supports_credit_preloads)
    {
        return Ok(CreditPlan {
            closure: initial,
            images: vec![],
            proofs: vec![],
        });
    }
    let parents = history.accounts(&initial.dependency_accounts, a.parent, &Roles::default())?;
    match assert_account_coverage(&parents, &a.raw[..=a.index as usize], a.parent) {
        Ok(()) => {
            return Ok(CreditPlan {
                closure: initial,
                images: vec![],
                proofs: vec![],
            })
        }
        Err(error) if error.code == "UNSUPPORTED_HISTORICAL_ACCOUNT" => {}
        Err(error) => return Err(error),
    }
    credits::plan(&parents, &a.raw, transactions, a.index, &initial)
}

pub(in super::super) fn apply(parents: &mut [Value], images: &[Value]) -> Result<(), Error> {
    for image in images {
        let parent = parents
            .iter_mut()
            .find(|p| p["pubkey"] == image["pubkey"])
            .ok_or_else(|| invalid("credit preload account disappeared"))?;
        for field in [
            "owner",
            "dataBase64",
            "executable",
            "rentEpoch",
            "sourceSlot",
            "presence",
        ] {
            if parent[field] != image[field] {
                return Err(invalid(format!("credit preload changed {field}")));
            }
        }
        parent["lamports"] = image["lamports"].clone();
    }
    Ok(())
}
