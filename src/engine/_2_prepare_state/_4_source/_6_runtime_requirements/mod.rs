use super::*;

pub(super) fn runtime_requirements(
    a: &crate::_2_prepare_state::analysis::Analysis,
    execution: &[u64],
    requested: &[requested::RequestedResolution],
    included: &[dependencies::SemanticTransaction],
    closure: &dependencies::SemanticClosureResult,
) -> Result<(Value, BTreeSet<String>, Roles), Error> {
    let mut requirements = instructions::requirements(&a.raw, execution, a.index)?;
    if !requested.is_empty() {
        let mut each = Vec::new();
        for value in requested {
            let mut raw = a.raw.clone();
            raw[a.index as usize] = value.raw.clone();
            each.push(instructions::requirements(&raw, &[a.index], a.index)?);
        }
        let mut uses = Vec::new();
        for key in ["recentBlockhashes", "slotHashes", "slotHistory"] {
            uses.extend(
                array(&requirements[key]["uses"], "sysvar uses")?
                    .iter()
                    .cloned(),
            );
            for requested_requirements in &each {
                uses.extend(
                    array(
                        &requested_requirements[key]["uses"],
                        "requested sysvar uses",
                    )?
                    .iter()
                    .cloned(),
                );
            }
        }
        requirements = instructions::classify(&uses, a.index)?;
    }
    let mut runtime_resolvable = BTreeSet::new();
    if requirements["recentBlockhashes"]["runtimeResolvable"] == true {
        runtime_resolvable.insert(RECENT_BLOCKHASHES.to_owned());
    }
    let roles = Roles {
        programs: included
            .iter()
            .flat_map(|t| t.program_ids.iter().cloned())
            .collect(),
        program_data: closure.program_data_accounts.iter().cloned().collect(),
        address_tables: included
            .iter()
            .flat_map(|t| t.address_table_accounts.iter().cloned())
            .collect(),
        sysvars: closure
            .dependency_accounts
            .iter()
            .filter(|k| k.starts_with("Sysvar"))
            .cloned()
            .chain([RENT.into(), RESTART.into()])
            .collect(),
        excluded: runtime_resolvable.clone(),
    };
    Ok((requirements, runtime_resolvable, roles))
}
