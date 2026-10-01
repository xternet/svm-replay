use super::*;

// Run after reviewed Bank initialization, before any transaction. An archive null
// may represent excluded data (e.g. vote accounts), not a truly empty account.
pub(in super::super) fn assert_account_coverage(
    accounts: &[Value],
    transactions: &[Value],
    parent_slot: u64,
) -> Result<(), Error> {
    let mut absent = BTreeSet::new();
    for account in accounts {
        if account["presence"] == "absent" && account["role"] == "application" {
            absent.insert(
                account["pubkey"]
                    .as_str()
                    .ok_or_else(|| invalid("absent account pubkey missing"))?
                    .to_owned(),
            );
        }
    }
    for (index, transaction) in transactions.iter().enumerate() {
        if absent.is_empty() {
            break;
        }
        let mut keys = strings(&transaction["transaction"]["message"]["accountKeys"])?;
        let meta = &transaction["meta"];
        if let Some(loaded) = meta.get("loadedAddresses").filter(|value| !value.is_null()) {
            keys.extend(strings(&loaded["writable"])?);
            keys.extend(strings(&loaded["readonly"])?);
        }
        let balances = array(&meta["preBalances"], "archived pre-balances")?;
        if balances.len() != keys.len() {
            return Err(invalid("archived pre-balance/account count differs"));
        }
        for (key, value) in keys.iter().zip(balances) {
            if !absent.remove(key) {
                continue;
            }
            let amount = crate::shared::diff::exact_u64(value).map_err(invalid)?;
            if amount != 0 {
                return Err(Error::new("UNSUPPORTED_HISTORICAL_ACCOUNT",
                    format!("{key}: archive returned no account image, but its first transaction has a nonzero pre-balance"))
                    .with_details(json!({"pubkey":key,"parentSlot":parent_slot,
                        "firstTransactionIndex":index,"historicalPreLamports":amount.to_string(),
                        "required":"exact historical owner, data and account fields; balance alone is insufficient"})));
            }
        }
    }
    if let Some(key) = absent.first() {
        return Err(
            Error::new(
                "UNSUPPORTED_HISTORICAL_ACCOUNT",
                format!("{key}: archive null has no independent historical absence evidence"),
            )
            .with_details(json!({"pubkey":key,"parentSlot":parent_slot,
                "reason":"unproven-account-absence",
                "required":"exact historical account image or independently established absence"})),
        );
    }
    Ok(())
}
