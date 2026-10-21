use super::*;
pub(in super::super) fn binding(value: &Value, slot: u64) -> Result<Vec<Value>, Error> {
    let mut seen = BTreeSet::new();
    array(value)?
        .iter()
        .map(|v| {
            let id = string(field(v, "id")?)?;
            key(id)?;
            let activation = number(field(v, "activationSlot")?)?;
            if activation > slot || !seen.insert(id) {
                return Err(invalid("invalid or duplicate active feature"));
            }
            Ok(json!({"id":id,"activationSlot":activation}))
        })
        .collect()
}
pub(in super::super) fn derive(
    features: &[Value],
    accounts: &serde_json::Map<String, Value>,
) -> Result<Value, Error> {
    if features.is_empty() {
        return Ok(json!({}));
    }
    let rent = accounts
        .get(RENT)
        .ok_or_else(|| Error::new("FEATURE_RENT", "feature accounts require historical Rent"))?;
    let data = bytes(field(rent, "dataBase64")?)?;
    if rent["owner"] != SYSVAR || rent["executable"] != false || data.len() != 17 {
        return Err(Error::new(
            "FEATURE_RENT",
            "invalid historical Rent account image",
        ));
    }
    let rate = u64::from_le_bytes(
        data[..8]
            .try_into()
            .map_err(|_| invalid("Rent rate width"))?,
    );
    let threshold = f64::from_le_bytes(
        data[8..16]
            .try_into()
            .map_err(|_| invalid("Rent threshold width"))?,
    );
    let yearly = rate
        .checked_mul(137)
        .ok_or_else(|| Error::new("FEATURE_RENT", "historical Rent overflow"))?;
    let minimum = yearly as f64 * threshold;
    if !threshold.is_finite()
        || threshold < 0.0
        || data[16] > 100
        || !minimum.is_finite()
        || !(0.0..18_446_744_073_709_551_616.0).contains(&minimum)
    {
        return Err(Error::new(
            "FEATURE_RENT",
            "historical Rent cannot produce u64 balance",
        ));
    }
    let mut images = serde_json::Map::new();
    for f in features {
        let mut data = vec![1];
        data.extend(number(field(f, "activationSlot")?)?.to_le_bytes());
        let id = string(field(f, "id")?)?;
        images.insert(id.to_owned(),json!({"pubkey":id,"owner":FEATURE,"executable":false,
            "rentEpoch":u64::MAX.to_string(),"lamports":(minimum as u64).to_string(),"dataBase64":STANDARD.encode(data)}));
    }
    Ok(Value::Object(images))
}
