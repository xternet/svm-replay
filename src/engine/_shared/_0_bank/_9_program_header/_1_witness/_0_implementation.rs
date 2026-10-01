use super::*;

fn keys(tx: &Value) -> Result<Vec<String>> {
    let mut keys = array(&tx["transaction"]["message"]["accountKeys"])?
        .iter()
        .map(|v| pubkey(v).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    if let Some(loaded) = tx["meta"].get("loadedAddresses").filter(|v| !v.is_null()) {
        for field in ["writable", "readonly"] {
            keys.extend(
                array(&loaded[field])?
                    .iter()
                    .map(|v| pubkey(v).map(str::to_owned))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    check(
        keys.iter().collect::<BTreeSet<_>>().len() == keys.len(),
        "duplicate message keys",
    )?;
    check(
        keys.len() == array(&tx["meta"]["preBalances"])?.len(),
        "missing pre-balances",
    )?;
    Ok(keys)
}
fn indexed<'a>(keys: &'a [String], index: &Value) -> Result<&'a str> {
    keys.get(integer(index)? as usize)
        .map(String::as_str)
        .ok_or_else(|| fail("instruction key index"))
}
fn writable(tx: &Value, index: usize) -> Result<bool> {
    let message = &tx["transaction"]["message"];
    let header = &message["header"];
    let signed = integer(&header["numRequiredSignatures"])? as usize;
    let static_len = array(&message["accountKeys"])?.len();
    let read_signed = integer(&header["numReadonlySignedAccounts"])? as usize;
    let read_unsigned = integer(&header["numReadonlyUnsignedAccounts"])? as usize;
    check(
        signed <= static_len && read_signed <= signed && read_unsigned <= static_len - signed,
        "message privileges",
    )?;
    Ok(if index < signed {
        index < signed - read_signed
    } else if index < static_len {
        index < static_len - read_unsigned
    } else {
        index < static_len + array(&tx["meta"]["loadedAddresses"]["writable"])?.len()
    })
}

pub(in super::super) fn upgrade(
    block: &Value,
    program: &str,
    data_key: &str,
    minimum: u64,
) -> Result<()> {
    let mut found = 0;
    for tx in array(&block["transactions"])? {
        if !field(&tx["meta"], "err")?.is_null() {
            continue;
        }
        let keys = keys(tx)?;
        let Some(index) = keys.iter().position(|k| k == program) else {
            continue;
        };
        let instructions = array(&tx["transaction"]["message"]["instructions"])?;
        let mut calls = Vec::new();
        for ix in instructions {
            if indexed(&keys, &ix["programIdIndex"])? == LOADER {
                calls.push(ix);
            }
        }
        if calls.len() != 1 {
            continue;
        }
        let call = calls[0];
        let data = bs58::decode(string(&call["data"])?)
            .into_vec()
            .map_err(|_| fail("native instruction encoding"))?;
        if data != 3u32.to_le_bytes() {
            continue;
        }
        let accounts = array(&call["accounts"])?;
        if accounts.len() < 7
            || indexed(&keys, &accounts[0])? != data_key
            || indexed(&keys, &accounts[1])? != program
        {
            continue;
        }
        // Only outer native success attests this check; a failed CPI can be
        // caught by a successful caller and must never become a witness.
        for ix in instructions {
            check(
                ix == call
                    || (indexed(&keys, &ix["programIdIndex"])?
                        == "ComputeBudget111111111111111111111111111111"
                        && array(&ix["accounts"])?.is_empty()),
                "ambiguous upgrade transaction",
            )?;
        }
        check(writable(tx, index)?, "Program not writable")?;
        check(
            uint(&tx["meta"]["preBalances"][index])? >= minimum
                && uint(&tx["meta"]["postBalances"][index])? >= minimum,
            "Program not exempt at upgrade",
        )?;
        found += 1;
    }
    check(found == 1, "unique successful native Upgrade required")
}

pub(in super::super) fn first_balance(block: &Value, program: &str) -> Result<u64> {
    check(
        !array(&block["rewards"])?
            .iter()
            .any(|r| r["pubkey"] == program),
        "Bank reward ambiguity",
    )?;
    for tx in array(&block["transactions"])? {
        if let Some(index) = keys(tx)?.iter().position(|k| k == program) {
            return uint(&tx["meta"]["preBalances"][index]);
        }
    }
    Err(fail("first-touch Program balance missing"))
}
