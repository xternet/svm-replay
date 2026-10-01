use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedAltParentProof {
    pub pubkey: String,
    pub parent: String,
    pub parent_slot: u64,
    pub target_slot: u64,
    pub target_index: u64,
    pub selected_writer_indices: Vec<u64>,
}

pub struct CreatedAltParents {
    pub parent_roles: Roles,
    pub created_tables: Vec<CreatedAltParentProof>,
}

pub struct CreatedAltInput<'a> {
    pub records: &'a [Value],
    pub roles: &'a Roles,
    pub parent_slot: u64,
    pub target_slot: u64,
    pub target_index: u64,
    pub selected: &'a [SemanticTransaction],
    pub raw_transactions: &'a [Value],
    pub requested_table_keys: &'a [String],
    pub overrides: &'a [RequestedAccountOverride],
}

pub fn prepare_created_alt_parents(input: CreatedAltInput<'_>) -> Result<CreatedAltParents> {
    let mut parent_roles = input.roles.clone();
    let mut created_tables = Vec::new();
    for edit in checked_overrides(input.overrides)? {
        if edit.data_base64.is_none()
            || !input.requested_table_keys.contains(&edit.pubkey)
            || !parent_roles.address_tables.contains(&edit.pubkey)
        {
            continue;
        }
        let matches: Vec<_> = input
            .records
            .iter()
            .filter(|record| record["pubkey"] == edit.pubkey)
            .collect();
        if matches.len() != 1 {
            return Err(invalid(format!(
                "MISSING_CREATED_ALT_PARENT:{}:expected one exact parent record",
                edit.pubkey
            )));
        }
        let value = matches[0];
        if value["sourceSlot"].as_u64() != Some(input.parent_slot) {
            return Err(Error::new(
                "SOURCE_CONTEXT_MISMATCH",
                format!("{}: created ALT parent slot mismatch", edit.pubkey),
            ));
        }
        if value["presence"] == "present" && value["owner"] == ALT {
            continue;
        }
        if value["presence"] != "absent"
            && (value["presence"] != "present"
                || value["owner"] != SYSTEM
                || value["executable"] != false
                || value["dataBase64"] != "")
        {
            return Err(invalid(format!(
                "INVALID_CREATED_ALT_BASELINE:{}:expected absent or empty System account",
                edit.pubkey
            )));
        }
        if input.roles.programs.contains(&edit.pubkey)
            || input.roles.program_data.contains(&edit.pubkey)
            || input.roles.sysvars.contains(&edit.pubkey)
            || input.roles.excluded.contains(&edit.pubkey)
        {
            return Err(invalid(format!(
                "INVALID_CREATED_ALT_BASELINE:{}:conflicting dependency role",
                edit.pubkey
            )));
        }
        let mut writers: Vec<_> = input
            .selected
            .iter()
            .filter(|tx| {
                tx.index < input.target_index && tx.writable_accounts.contains(&edit.pubkey)
            })
            .cloned()
            .collect();
        writers.sort_by_key(|writer| writer.index);
        if writers
            .windows(2)
            .any(|pair| pair[0].index == pair[1].index)
        {
            return Err(invalid(format!(
                "INVALID_CREATED_ALT_BASELINE:{}:duplicate selected writer",
                edit.pubkey
            )));
        }
        let created = assert_requested_parent_alt_still_usable(
            &edit.pubkey,
            &writers,
            input.parent_slot,
            &RequestedBoundaryEvidence {
                target_slot: input.target_slot,
                raw_transactions: input.raw_transactions.to_vec(),
            },
        )?;
        if !created {
            return Err(invalid(format!(
                "INVALID_CREATED_ALT_BASELINE:{}:committed selected Create evidence required",
                edit.pubkey
            )));
        }
        parent_roles.address_tables.remove(&edit.pubkey);
        created_tables.push(CreatedAltParentProof {
            pubkey: edit.pubkey,
            parent: if value["presence"] == "absent" {
                "absent".into()
            } else {
                "prefunded-system".into()
            },
            parent_slot: input.parent_slot,
            target_slot: input.target_slot,
            target_index: input.target_index,
            selected_writer_indices: writers.iter().map(|writer| writer.index).collect(),
        });
    }
    Ok(CreatedAltParents {
        parent_roles,
        created_tables,
    })
}
