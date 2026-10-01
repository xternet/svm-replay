use super::*;

/// Native loader account bytes affect historical loaded-data admission limits.
pub(in super::super) fn include_loaders(
    records: &mut Vec<Value>,
    mut fetch: impl FnMut(&str) -> Result<Value, Error>,
) -> Result<(), Error> {
    let owners = records
        .iter()
        .filter(|a| a["presence"] == "present" && a["executable"] == true)
        .filter_map(|a| a["owner"].as_str())
        .filter(|owner| {
            matches!(
                *owner,
                "BPFLoader1111111111111111111111111111111111"
                    | "BPFLoader2111111111111111111111111111111111"
                    | "BPFLoaderUpgradeab1e11111111111111111111111"
                    | "LoaderV411111111111111111111111111111111111"
            )
        })
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for owner in owners {
        if !records.iter().any(|a| a["pubkey"] == owner) {
            records.push(fetch(&owner)?);
        }
        let account = required(records, &owner)?;
        if account["presence"] != "present" {
            return Err(Error::new(
                "UNSUPPORTED_HISTORICAL_ACCOUNT",
                format!("exact historical implicit loader account required: {owner}"),
            ));
        }
    }
    Ok(())
}
