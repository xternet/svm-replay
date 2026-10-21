use super::*;
use std::collections::BTreeSet;

type OwnerQuery = (String, Option<String>, Option<String>);

/// Pages of one owner's at-slot list read before the owner is refused as too large to list.
const MAX_PAGES: u64 = 100;

fn invalid(message: &str) -> Error {
    Error::new("INVALID_QUERY", message)
}

fn pubkey(value: &str, what: &str) -> Result<(), Error> {
    base58(&json!(value), 32, what)
        .map(drop)
        .map_err(|_| invalid(what))
}

/// The slot whose end state precedes the boundary block: provider lists are read there.
fn parent(block: &Value) -> Result<u64, Error> {
    block["parentSlot"]
        .as_u64()
        .ok_or_else(|| integrity("boundary parent slot missing"))
}

/// An at-slot answer counts, for presence or absence alike, only for the slot it was asked
/// about, as historical account reads require.
fn at_slot(answer: &Value, slot: u64, what: &str) -> Result<(), Error> {
    match answer["context"]["slot"].as_u64() == Some(slot) {
        true => Ok(()),
        false => Err(Error::new(
            "SOURCE_CONTEXT_MISMATCH",
            format!("{what} is not for the parent slot {slot}"),
        )),
    }
}

/// Every token balance before and after each transaction of the boundary prefix, with its
/// account key. Metadata without token balances cannot show what the prefix changed.
fn prefix_balances(block: &Value, position: usize) -> Result<Vec<(String, &Value)>, Error> {
    let mut found = Vec::new();
    for entry in &transactions(block)?[..position] {
        let keys = keys(entry)?;
        for side in ["preTokenBalances", "postTokenBalances"] {
            let balances = entry["meta"][side]
                .as_array()
                .ok_or_else(|| integrity("archived token balances missing"))?;
            for balance in balances {
                let key = balance["accountIndex"]
                    .as_u64()
                    .and_then(|index| keys.get(index as usize))
                    .ok_or_else(|| integrity("archived token balance account index"))?;
                found.push((key.clone(), balance));
            }
        }
    }
    Ok(found)
}

/// The token accounts that may be `owner`'s at the position: every page of the provider's
/// list at the parent slot, each checked against the query, and every account the prefix's
/// token balances show the owner held before or after a transaction.
pub(in super::super) fn owner_candidates(
    session: &mut impl Archive,
    block: &Value,
    position: usize,
    (owner, mint, program_id): OwnerQuery,
) -> Result<Value, Error> {
    if mint.is_some() == program_id.is_some() {
        return Err(invalid(
            "a token account filter names exactly one of mint or programId",
        ));
    }
    pubkey(&owner, "token account owner")?;
    for filter in mint.iter().chain(&program_id) {
        pubkey(filter, "token account filter")?;
    }
    let slot = parent(block)?;
    let (mut accounts, mut keys, mut pages) = (BTreeSet::new(), BTreeSet::new(), 0);
    let mut page_key = Value::Null;
    loop {
        if pages == MAX_PAGES {
            return Err(Error::new(
                "SOURCE_RESOURCE_LIMIT",
                "the owner's token accounts exceed the listed page limit",
            ));
        }
        let listed = session.archive(&json!({"kind":"tokenAccountsByOwner","owner":owner,
            "mint":mint,"programId":program_id,"slot":slot,"pageKey":page_key}))?;
        pages += 1;
        at_slot(&listed, slot, "a listed token account page")?;
        for item in listed["value"]
            .as_array()
            .ok_or_else(|| integrity("listed token accounts missing"))?
        {
            let info = &item["account"]["data"]["parsed"]["info"];
            let matches = info["owner"] == owner
                && mint.as_ref().is_none_or(|mint| info["mint"] == *mint)
                && program_id
                    .as_ref()
                    .is_none_or(|program| item["account"]["owner"] == *program);
            if !matches {
                return Err(integrity("a listed token account contradicts its query"));
            }
            accounts.insert(base58(&item["pubkey"], 32, "listed token account")?);
        }
        page_key = match &listed["pageKey"] {
            Value::Null => break,
            Value::String(next) if !next.is_empty() && keys.insert(next.clone()) => json!(next),
            _ => return Err(integrity("listed token account pagination")),
        };
    }
    // A balance's owner is optional metadata: without it the account may be the owner's.
    for (key, balance) in prefix_balances(block, position)? {
        let matches = (balance["owner"].is_null() || balance["owner"] == owner)
            && mint.as_ref().is_none_or(|mint| balance["mint"] == *mint)
            && program_id.as_ref().is_none_or(|program| {
                balance["programId"].is_null() || balance["programId"] == *program
            });
        if matches {
            accounts.insert(key);
        }
    }
    Ok(json!({"accounts":accounts,"slot":slot,"pages":pages}))
}

/// `mint`'s nonzero token accounts, largest first, at the parent slot: complete when the
/// provider returned fewer than `limit`, otherwise no unlisted account exceeds `floor`.
/// `touched` names the accounts the prefix changed, whose ranked balances may be stale. A
/// mint absent at the parent slot has no accounts there: every one is in `touched` or local.
pub(in super::super) fn holders(
    session: &mut impl Archive,
    block: &Value,
    position: usize,
    (mint, limit): (String, u64),
) -> Result<Value, Error> {
    pubkey(&mint, "token holder mint")?;
    if !(1..=10_000).contains(&limit) {
        return Err(invalid("token holder limit must be 1..=10000"));
    }
    let slot = parent(block)?;
    let touched: BTreeSet<String> = prefix_balances(block, position)?
        .into_iter()
        .filter(|(_, balance)| balance["mint"] == mint)
        .map(|(key, _)| key)
        .collect();
    let image = session.archive(&json!({"kind":"accountAtSlot","pubkey":mint,"slot":slot}))?;
    at_slot(&image, slot, "the mint's image")?;
    match &image["value"] {
        Value::Object(_) => {}
        Value::Null => {
            return Ok(
                json!({"slot":slot,"mintAtParent":false,"holders":[],"complete":true,
                "floor":null,"touched":touched}),
            )
        }
        _ => return Err(integrity("the mint's image at the parent slot")),
    }
    let ranked =
        session.archive(&json!({"kind":"tokenHolders","mint":mint,"slot":slot,"limit":limit}))?;
    if ranked["mint"] != mint || ranked["slot"] != slot {
        return Err(integrity("the holder ranking is for another mint or slot"));
    }
    let list = ranked["holders"]
        .as_array()
        .filter(|list| list.len() as u64 <= limit)
        .ok_or_else(|| integrity("the holder ranking is missing or exceeds its limit"))?;
    let (mut holders, mut previous) = (Vec::with_capacity(list.len()), u64::MAX);
    for holder in list {
        let amount = holder["balanceRaw"]
            .as_str()
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|amount| (1..=previous).contains(amount))
            .ok_or_else(|| integrity("the holder ranking is not nonzero and largest first"))?;
        previous = amount;
        holders.push(
            json!({"account":base58(&holder["holder"], 32, "holder account")?,
            "amount":amount.to_string()}),
        );
    }
    let complete = (list.len() as u64) < limit;
    let floor = match complete {
        true => Value::Null,
        false => holders
            .last()
            .map_or(Value::Null, |last| last["amount"].clone()),
    };
    Ok(
        json!({"slot":slot,"mintAtParent":true,"holders":holders,"complete":complete,
        "floor":floor,"touched":touched}),
    )
}
