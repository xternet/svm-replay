use super::*;

/// Callback returns exact normalized historical accounts, including explicit absent records.
#[allow(clippy::too_many_arguments)]
pub fn resolve_requested_dependencies_with_accounts<F>(
    encoded: &str,
    original: &Value,
    transactions: &[SemanticTransaction],
    target_index: u64,
    parent_slot: u64,
    mut accounts: F,
    boundary: Option<&RequestedBoundaryEvidence>,
    overrides: &[RequestedAccountOverride],
) -> Result<RequestedResolution>
where
    F: FnMut(&[String], u64) -> Result<Vec<Value>>,
{
    safe_integer(target_index, "target index")?;
    safe_integer(parent_slot, "parent slot")?;
    let decoded = decode(encoded)?;
    object(original, "original transaction")?;
    object(&original["transaction"], "original payload")?;
    let original_message = &original["transaction"]["message"];
    object(original_message, "original message")?;
    object(&original["meta"], "original meta")?;
    let original_lookups = match original_message.get("addressTableLookups") {
        None => Vec::new(),
        Some(value) => array(value, "original lookups")?.clone(),
    };
    let lookups = array(
        &decoded["transaction"]["message"]["addressTableLookups"],
        "decoded lookups",
    )?;
    let checked = checked_overrides(overrides)?;
    let edits: BTreeMap<_, _> = checked
        .iter()
        .filter_map(|entry| {
            entry
                .data_base64
                .as_ref()
                .filter(|_| {
                    lookups
                        .iter()
                        .any(|lookup| lookup["accountKey"] == entry.pubkey)
                })
                .map(|data| (entry.pubkey.as_str(), data.as_str()))
        })
        .collect();
    if !edits.is_empty() && boundary.is_none() {
        return Err(invalid(
            "ALT override resolution requires the exact target boundary",
        ));
    }
    let mut writable = Vec::new();
    let mut readonly = Vec::new();
    if lookups.is_empty() {
        // Legacy/no-lookup messages have no dynamically loaded address section.
    } else if edits.is_empty() && *lookups == original_lookups {
        let loaded = &original["meta"]["loadedAddresses"];
        object(loaded, "original loaded addresses")?;
        writable = pubkeys(&loaded["writable"], "loaded writable")?;
        readonly = pubkeys(&loaded["readonly"], "loaded readonly")?;
        let count = |field: &str| -> Result<usize> {
            lookups
                .iter()
                .map(|lookup| array(&lookup[field], "lookup indexes").map(Vec::len))
                .sum()
        };
        if writable.len() != count("writableIndexes")?
            || readonly.len() != count("readonlyIndexes")?
        {
            return Err(invalid("original loaded address cardinality mismatch"));
        }
    } else {
        let table_keys = unique(
            lookups
                .iter()
                .map(|lookup| string(&lookup["accountKey"], "lookup key").map(String::from))
                .collect::<Result<Vec<_>>>()?,
        );
        let changing: Vec<_> = transactions
            .iter()
            .filter(|tx| {
                tx.index < target_index
                    && tx.succeeded
                    && tx
                        .writable_accounts
                        .iter()
                        .any(|key| table_keys.contains(key))
            })
            .cloned()
            .collect();
        if !changing.is_empty() && boundary.is_none() {
            return Err(unsupported("replacement-only lookup resolution needs a same-slot table boundary, not a parent snapshot"));
        }
        let mut created = BTreeSet::new();
        if let Some(evidence) = boundary {
            for table in &table_keys {
                let writers: Vec<_> = changing
                    .iter()
                    .filter(|tx| tx.writable_accounts.contains(table))
                    .cloned()
                    .collect();
                if assert_requested_parent_alt_still_usable(table, &writers, parent_slot, evidence)?
                {
                    created.insert(table.clone());
                }
            }
        }
        let snapshots = accounts(&table_keys, parent_slot)?;
        let mut tables = BTreeMap::new();
        for record in snapshots {
            let key = string(&record["pubkey"], "historical table pubkey")?.to_owned();
            if record["sourceSlot"].as_u64() != Some(parent_slot) || !table_keys.contains(&key) {
                return Err(Error::new(
                    "SOURCE_CONTEXT_MISMATCH",
                    format!("{key}: requested table parent coordinate mismatch"),
                ));
            }
            if record["presence"] != "present" && record["presence"] != "absent" {
                return Err(Error::new(
                    "SOURCE_INTEGRITY",
                    format!("{key}: explicit historical presence required"),
                ));
            }
            if tables.insert(key.clone(), record).is_some() {
                return Err(Error::new(
                    "SOURCE_INTEGRITY",
                    format!("{key}: duplicate historical table record"),
                ));
            }
        }
        for lookup in lookups {
            let key = string(&lookup["accountKey"], "lookup key")?;
            let edited = edits.get(key).copied();
            let record = tables.get(key);
            let absent = record.is_some_and(|record| record["presence"] == "absent");
            let created_override = absent && created.contains(key) && edited.is_some();
            if record.is_none() || (absent && !created_override) {
                let indexes: Vec<_> = array(&lookup["writableIndexes"], "lookup writable indexes")?
                    .iter()
                    .chain(array(
                        &lookup["readonlyIndexes"],
                        "lookup readonly indexes",
                    )?)
                    .collect();
                if absent && created.contains(key) && !indexes.is_empty() {
                    return Err(invalid(format!(
                        "INVALID_REQUESTED_LOOKUP_INDEX:{key}:{}: table created in current slot",
                        indexes[0]
                    )));
                }
                return Err(unsupported(format!(
                    "{key}: exact replacement lookup table unavailable"
                )));
            }
            let record = record.ok_or_else(|| {
                unsupported(format!("{key}: exact replacement lookup table unavailable"))
            })?;
            let created_prefunded = edited.is_some()
                && created.contains(key)
                && !absent
                && record["owner"] == SYSTEM
                && record["executable"] == false
                && record["dataBase64"] == "";
            if !absent
                && !created_prefunded
                && (record["owner"] != ALT
                    || record["executable"] != false
                    || !record["dataBase64"].is_string())
            {
                return Err(invalid(format!(
                    "{key}: malformed replacement lookup table"
                )));
            }
            let encoded = match edited {
                Some(encoded) => encoded,
                None => string(&record["dataBase64"], "historical table data")?,
            };
            let data = bytes(
                encoded,
                &format!("{key}: invalid replacement lookup table state"),
            )?;
            let latest = if edited.is_some() {
                boundary
                    .ok_or_else(|| invalid("ALT override boundary missing"))?
                    .target_slot
            } else {
                parent_slot
            };
            if data.len() < 56
                || (data.len() - 56) % 32 != 0
                || data.len() > 56 + 256 * 32
                || u32_at(&data, 0)? != 1
                || data[21] > 1
                || usize::from(data[20]) > (data.len() - 56) / 32
                || u64_at(&data, 12)? > latest
            {
                return Err(invalid(format!(
                    "{key}: invalid replacement lookup table state"
                )));
            }
            let active = if edited.is_some()
                && u64_at(&data, 12)?
                    == boundary
                        .ok_or_else(|| invalid("ALT override boundary missing"))?
                        .target_slot
            {
                usize::from(data[20])
            } else {
                (data.len() - 56) / 32
            };
            for (field, destination) in [
                ("writableIndexes", &mut writable),
                ("readonlyIndexes", &mut readonly),
            ] {
                for raw_index in array(&lookup[field], "lookup indexes")? {
                    let i = index(raw_index, "lookup index")?;
                    if i >= active as u64 {
                        return Err(invalid(format!("INVALID_REQUESTED_LOOKUP_INDEX:{key}:{i}: absent or same-slot cold index")));
                    }
                    let offset = 56 + i as usize * 32;
                    destination.push(bs58::encode(&data[offset..offset + 32]).into_string());
                }
            }
        }
    }
    let mut raw = decoded;
    raw["meta"] = json!({"err":null,"loadedAddresses":{"writable":writable,"readonly":readonly},"innerInstructions":[]});
    let summary = summarize_semantic_transaction(&raw, target_index)?;
    Ok(RequestedResolution { raw, summary })
}
