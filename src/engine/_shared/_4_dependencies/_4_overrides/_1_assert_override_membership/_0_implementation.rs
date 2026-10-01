use super::*;

pub fn assert_override_membership(
    overrides: &[RequestedAccountOverride],
    requested_keys: &[String],
    table_keys: &[String],
    accounts: &[Value],
) -> Result<()> {
    let mut backing = BTreeSet::new();
    for account in accounts {
        let Some(key) = account["pubkey"].as_str() else {
            return Err(invalid("account pubkey missing"));
        };
        if account["presence"] != "present"
            || account["executable"] != true
            || account["owner"] != LOADER
            || !requested_keys.iter().any(|k| k == key)
        {
            continue;
        }
        let encoded = account["dataBase64"]
            .as_str()
            .ok_or_else(|| invalid(format!("program bytes missing:{key}")))?;
        let data = bytes(encoded, &format!("program bytes malformed:{key}"))?;
        // Matching the prototype's inspection link recognizer is not loader admission.
        if data.len() >= 36 && u32_at(&data, 0)? == 2 {
            backing.insert(bs58::encode(&data[4..36]).into_string());
        }
    }
    for entry in overrides {
        if entry.pubkey == "Sysvar1nstructions1111111111111111111111111" {
            return Err(invalid("RUNTIME_DERIVED_SYSVAR_OVERRIDE"));
        }
        if !requested_keys.contains(&entry.pubkey)
            && !table_keys.contains(&entry.pubkey)
            && !OVERRIDABLE_SYSVARS.contains(&entry.pubkey.as_str())
            && !backing.contains(&entry.pubkey)
        {
            return Err(invalid(format!(
                "UNRELATED_ACCOUNT_OVERRIDE:{}",
                entry.pubkey
            )));
        }
    }
    Ok(())
}

pub fn requested_image_inspection(
    accounts: &[Value],
    overrides: &[RequestedAccountOverride],
) -> Result<Vec<Value>> {
    let changes: BTreeMap<_, _> = overrides
        .iter()
        .map(|entry| (entry.pubkey.as_str(), entry))
        .collect();
    let mut inspected = Vec::new();
    for account in accounts {
        let key = account["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("account pubkey missing"))?;
        let mut image = account.clone();
        if let Some(encoded) = changes
            .get(key)
            .and_then(|entry| entry.data_base64.as_deref())
        {
            if account["presence"] == "present"
                && (account["executable"] == true
                    || (account["owner"] == LOADER && account["role"] == "programdata"))
            {
                let before = bytes(
                    account["dataBase64"]
                        .as_str()
                        .ok_or_else(|| invalid(format!("program bytes missing:{key}")))?,
                    &format!("program bytes malformed:{key}"),
                )?;
                let after = bytes(
                    encoded,
                    &format!("INVALID_PROGRAM_OVERRIDE:{key}:image encoding"),
                )?;
                if account["owner"] == LOADER {
                    if account["executable"] == true {
                        if before != after {
                            return Err(invalid(format!(
                                "INVALID_PROGRAM_OVERRIDE:{key}:ProgramData relationship"
                            )));
                        }
                    } else if before.len() < 45
                        || after.len() < 49
                        || u32_at(&after, 0)? != 3
                        || after[12] > 1
                        || &after[45..49] != b"\x7fELF"
                    {
                        return Err(invalid(format!(
                            "INVALID_PROGRAM_OVERRIDE:{key}:ProgramData metadata/image"
                        )));
                    }
                } else if !after.starts_with(b"\x7fELF") {
                    return Err(invalid(format!("INVALID_PROGRAM_OVERRIDE:{key}:ELF image")));
                }
                image["dataBase64"] = Value::String(encoded.to_owned());
            }
        }
        inspected.push(image);
    }
    Ok(inspected)
}
