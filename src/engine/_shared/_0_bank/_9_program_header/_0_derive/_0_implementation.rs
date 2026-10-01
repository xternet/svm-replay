use super::*;

/// Raw provider inputs must already be provenance-checked. Future metadata is
/// retained as future evidence, never relabeled as a target-slot observation.
pub fn derive(input: &Value) -> Result<Value> {
    let program = pubkey(&input["program"])?;
    check(
        !BANK_MANAGED.contains(&program),
        "Bank-managed program needs migration proof",
    )?;
    let parent = integer(&input["parentSlot"])?;
    let target = integer(&input["targetSlot"])?;
    let witness = integer(&input["witnessSlot"])?;
    let anchor = &input["anchor"];
    let later = integer(&anchor["sourceSlot"])?;
    check(
        witness < parent && parent < target && target <= later,
        "header evidence order",
    )?;
    check(
        input["targetBlock"]["parentSlot"] == parent,
        "wrong parent block",
    )?;
    review(&input["registry"], witness, later)?;
    check(
        anchor["pubkey"] == program && anchor["executable"] == true,
        "wrong Program anchor",
    )?;
    let header = account_bytes(anchor, LOADER, later)?;
    check(
        header.len() == 36 && header[..4] == 2u32.to_le_bytes(),
        "Program header shape",
    )?;
    let data_key = bs58::encode(&header[4..]).into_string();
    let data = &input["programdata"];
    check(
        data["pubkey"] == data_key && data["executable"] == false,
        "ProgramData link/flag",
    )?;
    let code = account_bytes(data, LOADER, parent)?;
    check(
        code.len() >= 45 && code[..4] == 3u32.to_le_bytes(),
        "ProgramData shape",
    )?;
    let deployed = u64::from_le_bytes(
        code[4..12]
            .try_into()
            .map_err(|_| fail("deployment slot"))?,
    );
    check(deployed == witness, "wrong deployment witness")?;
    let minimum = rent_minimum(&input["rent"], parent)?;
    upgrade(&input["witnessBlock"], program, &data_key, minimum)?;
    let balance = first_balance(&input["targetBlock"], program)?;
    check(balance >= minimum, "Program no longer exempt")?;
    let account = json!({"pubkey":program,"sourceSlot":parent,"role":"program","presence":"present",
        "owner":LOADER,"executable":true,"dataBase64":anchor["dataBase64"],
        "lamports":balance.to_string(),"rentEpoch":u64::MAX.to_string()});
    Ok(
        json!({"account":account,"proof":{"schema":"historical-program-header/v1",
        "pubkey":program,"parentSlot":parent,"targetSlot":target,"anchorSlot":later,
        "witnessSlot":witness,"programdata":data_key,"rentMinimum":minimum.to_string(),
        "inputsSha256":hash(canonical_json(input)),"accountSha256":hash(canonical_json(&account))}}),
    )
}
