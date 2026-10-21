use super::*;
use std::collections::BTreeSet;

type Listing = (String, Option<String>, Option<String>, u64);

/// How far an empty boundary block looks back for the nearest earlier block holding a
/// transaction: produced-slot windows and block reads, both bounded.
const ANCHOR_WINDOW: u64 = 1_000;
const ANCHOR_WINDOWS: usize = 4;
const ANCHOR_BLOCKS: usize = 16;

/// The fork's signature list for an address: the boundary block's own entries before the
/// position, newest first, then earlier archive pages, checked entry by entry.
pub(in super::super) fn signatures(
    session: &mut impl Archive,
    block: &Value,
    slot: u64,
    position: usize,
    listing: Listing,
) -> Result<Value, Error> {
    let (address, before, until, limit) = listing;
    if !(1..=1000).contains(&limit) {
        return Err(Error::new(
            "INVALID_QUERY",
            "signature limit must be 1..=1000",
        ));
    }
    let names = signatures_of(block)?;
    let cursor = before
        .as_ref()
        .map(|before| names.iter().position(|name| name == before));
    // A cursor older than the boundary block skips its entries; one at or after the position
    // did not happen on this fork (an unknown cursor lists nothing, as the RPC does).
    let newest = match cursor {
        None => position,
        Some(Some(index)) if index >= position => return Ok(json!([])),
        Some(Some(index)) => index,
        Some(None) => 0,
    };
    let mut entries = Vec::new();
    let (until, mut seen) = (until.as_ref(), BTreeSet::from_iter(names.iter().cloned()));
    if list(
        &mut entries,
        block,
        slot,
        &names[..newest],
        &address,
        until,
        limit,
    )? {
        return Ok(json!(entries));
    }
    // Earlier history pages strictly before the boundary block's first transaction (or an
    // older cursor). An empty boundary block anchors at the nearest earlier block holding a
    // transaction: all of that block precedes the position.
    let (older, mut newer) = match (cursor, names.first()) {
        (Some(None), _) => (before.clone(), slot),
        (_, Some(first)) => (Some(first.clone()), slot),
        (_, None) => {
            let (at, anchor) = anchor(session, slot)?;
            let anchored = signatures_of(&anchor)?;
            seen.extend(anchored.iter().cloned());
            if list(&mut entries, &anchor, at, &anchored, &address, until, limit)? {
                return Ok(json!(entries));
            }
            (Some(anchored[0].clone()), at)
        }
    };
    let remaining = limit - entries.len() as u64;
    let page = session.archive(&json!({"kind":"signatures","address":address,"before":older,"until":until,"limit":remaining}))?;
    let page = page
        .as_array()
        .ok_or_else(|| integrity("archived signatures missing"))?;
    if page.len() as u64 > remaining {
        return Err(integrity("archived signatures exceed their limit"));
    }
    for entry in page {
        let name = base58(&entry["signature"], 64, "archived signature")?;
        let at = entry["slot"]
            .as_u64()
            .filter(|at| *at < slot && *at <= newer);
        let shaped = entry.get("err").is_some()
            && (entry["blockTime"].is_null() || entry["blockTime"].is_i64())
            && (entry["memo"].is_null() || entry["memo"].is_string())
            && entry["confirmationStatus"] == "finalized";
        if at.is_none() || !shaped || !seen.insert(name) {
            return Err(integrity("archived signatures are not finalized history before the boundary, newest first, each once"));
        }
        newer = at.unwrap_or(newer);
    }
    entries.extend(page.iter().cloned());
    Ok(json!(entries))
}

fn signatures_of(block: &Value) -> Result<Vec<String>, Error> {
    transactions(block)?.iter().map(signature).collect()
}

/// Appends the entries for `address` among `block`'s first transactions (`names`), newest
/// first; true once `until` or `limit` ends the listing.
fn list(
    entries: &mut Vec<Value>,
    block: &Value,
    slot: u64,
    names: &[String],
    address: &str,
    until: Option<&String>,
    limit: u64,
) -> Result<bool, Error> {
    let listed = transactions(block)?[..names.len()].iter().zip(names);
    for (index, (entry, name)) in listed.enumerate().rev() {
        if !keys(entry)?.iter().any(|key| key == address) {
            continue;
        }
        if until == Some(name) {
            return Ok(true);
        }
        entries.push(
            json!({"signature":name,"slot":slot,"err":entry["meta"]["err"],"memo":memo(entry)?,
            "blockTime":block["blockTime"],"confirmationStatus":"finalized","transactionIndex":index}),
        );
        if entries.len() as u64 == limit {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The nearest produced block before `slot` holding a transaction, read through the
/// pinned archive within the anchor bounds.
fn anchor(session: &mut impl Archive, slot: u64) -> Result<(u64, Value), Error> {
    let (mut end, mut reads) = (slot, 0);
    for _ in 0..ANCHOR_WINDOWS {
        if end == 0 {
            break;
        }
        let start = end.saturating_sub(ANCHOR_WINDOW);
        let listed = session.archive(&json!({"kind":"blocks","start":start,"end":end - 1}))?;
        for at in ascending(&listed, start, end - 1)?.into_iter().rev() {
            let block = session.archived_block(at)?;
            if !transactions(&block)?.is_empty() {
                return Ok((at, block));
            }
            reads += 1;
            if reads == ANCHOR_BLOCKS {
                return Err(unanchored(slot));
            }
        }
        end = start;
    }
    Err(unanchored(slot))
}

fn unanchored(slot: u64) -> Error {
    Error::new(
        "ARCHIVE_CURSOR_UNAVAILABLE",
        format!("no produced block with a transaction within the bounded search before slot {slot}; this bound is SVM Replay's archive paging limit, not missing history"),
    )
}
