use super::*;
pub(super) fn parent_keys(
    closure: &dependencies::SemanticClosureResult,
    initialized: &[Value],
    runtime_resolvable: &BTreeSet<String>,
) -> Result<Vec<String>, Error> {
    let mut parent_keys = closure.dependency_accounts.clone();
    parent_keys.extend(
        initialized
            .iter()
            .map(|v| {
                v["pubkey"]
                    .as_str()
                    .ok_or_else(|| invalid("migration pubkey"))
                    .map(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    parent_keys.extend([RENT.into(), RESTART.into()]);
    let parent_keys = unique(parent_keys)
        .into_iter()
        .filter(|key| key != CLOCK && key != INSTRUCTIONS && !runtime_resolvable.contains(key))
        .collect::<Vec<_>>();
    if parent_keys.len() + 2 > 3500 {
        return Err(Error::new(
            "UNSUPPORTED_RESOURCE_LIMIT",
            "more than 3500 historical accounts",
        ));
    }
    Ok(parent_keys)
}
