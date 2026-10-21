use super::*;
use overrides::RequestedAccountOverride;
use svm_replay_protocol::StateOverride;

const LOADER_V3: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// Account images after the requested changes, for program import inspection.
pub(in super::super::super) fn inventory_images(
    accounts: &[Value],
    changes: &[RequestedAccountOverride],
    input: &HistoricalRequest,
) -> Result<Vec<Value>, Error> {
    let mut images = overrides::requested_image_inspection(accounts, changes)?;
    let Some(what_if) = &input.what_if else {
        return Ok(images);
    };
    for entry in &what_if.state_overrides {
        match images
            .iter_mut()
            .find(|image| image["pubkey"] == entry.pubkey.as_str())
        {
            Some(image) => apply(image, entry),
            None => {
                let mut image = json!({"pubkey":entry.pubkey,"presence":"absent"});
                apply(&mut image, entry);
                images.push(image);
            }
        }
    }
    Ok(images)
}

fn apply(image: &mut Value, entry: &StateOverride) {
    if entry.lamports.as_deref() == Some("0") {
        image["presence"] = json!("absent");
        return;
    }
    if entry.owner.is_some() {
        image["executable"] = json!(false);
        image["dataBase64"] = json!("");
        image["role"] = json!("application");
    }
    image["presence"] = json!("present");
    let fields = [
        ("lamports", entry.lamports.as_ref().map(|v| json!(v))),
        ("owner", entry.owner.as_ref().map(|v| json!(v))),
        ("executable", entry.executable.map(|v| json!(v))),
        ("rentEpoch", entry.rent_epoch.as_ref().map(|v| json!(v))),
        ("dataBase64", entry.data_base64.as_ref().map(|v| json!(v))),
    ];
    for (name, value) in fields {
        if let Some(value) = value {
            image[name] = value;
        }
    }
    let programdata = image["dataBase64"]
        .as_str()
        .and_then(|data| STANDARD.decode(data).ok())
        .is_some_and(|data| data.starts_with(&3_u32.to_le_bytes()));
    if image["owner"] == LOADER_V3 && image["executable"] != true && programdata {
        image["role"] = json!("programdata");
    }
}

/// The variant fields of the worker fixture. A what-if bundle always lists its transactions,
/// and whole-account overrides carry every field, so the worker never reads the account first.
pub(in super::super::super) fn insert_variants(
    fixture: &mut Value,
    input: &HistoricalRequest,
    original_wire: &str,
    accounts: &[Value],
) -> Result<(), Error> {
    if let Some(value) = &input.requested_account_overrides {
        fixture["requestedAccountOverrides"] = value.clone();
    }
    let Some(what_if) = &input.what_if else {
        return Ok(());
    };
    let mut entry = what_if.clone();
    entry
        .transactions_base64
        .get_or_insert_with(|| vec![original_wire.to_owned()]);
    for state in &mut entry.state_overrides {
        if state.lamports.as_deref() == Some("0") {
            // A deleted (or declared-new) account has no state; the worker wants an owner for
            // an account that does not exist yet, and removes it either way.
            state.owner.get_or_insert_with(|| SYSTEM_PROGRAM.to_owned());
            continue;
        }
        if state.owner.is_none() {
            continue;
        }
        let data = state.data_base64.get_or_insert_with(String::new);
        let length = STANDARD
            .decode(data.as_str())
            .map_err(|e| invalid(format!("override data: {e}")))?
            .len();
        if state.lamports.is_none() {
            state.lamports = Some(rent_exempt_minimum(accounts, length)?.to_string());
        }
        state.executable.get_or_insert(false);
        state.rent_epoch.get_or_insert_with(|| u64::MAX.to_string());
    }
    fixture["whatIf"] = serde_json::to_value(entry).map_err(|e| invalid(format!("whatIf: {e}")))?;
    Ok(())
}

/// The historical Rent sysvar's minimum balance for `length` data bytes.
fn rent_exempt_minimum(accounts: &[Value], length: usize) -> Result<u64, Error> {
    let rent = required(accounts, RENT)?;
    let data = STANDARD
        .decode(rent["dataBase64"].as_str().unwrap_or_default())
        .map_err(|e| invalid(format!("Rent sysvar data: {e}")))?;
    if data.len() < 17 {
        return Err(invalid("Rent sysvar data is too short"));
    }
    let per_byte_year = u64::from_le_bytes(data[0..8].try_into().expect("8 bytes"));
    let threshold = f64::from_le_bytes(data[8..16].try_into().expect("8 bytes"));
    let bytes = (128 + length as u64)
        .checked_mul(per_byte_year)
        .ok_or_else(|| invalid("rent overflow"))?;
    Ok((bytes as f64 * threshold) as u64)
}

/// A bundle may use accounts that do not exist yet; the archive cannot prove that, the user can.
pub(in super::super::super) fn coverage_hint(error: Error, input: &HistoricalRequest) -> Error {
    let key = error.details.as_ref().and_then(|d| {
        (d["reason"] == "unproven-account-absence").then(|| d["pubkey"].as_str().map(str::to_owned))
    });
    match (input.what_if.is_some(), key.flatten()) {
        (true, Some(key)) => Error {
            message: format!(
                "{}. If your transactions create it, declare it new: --overrides '[{{\"pubkey\":\"{key}\",\"lamports\":\"0\"}}]'",
                error.message
            ),
            ..error
        },
        _ => error,
    }
}
