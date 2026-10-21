use super::*;

/// One address-history page request, as `ArchiveQuery::TransactionsForAddress` carries it.
pub(in super::super) struct AddressQuery {
    pub address: String,
    pub order: SortOrder,
    pub token_accounts: TokenAccounts,
    pub status: Status,
    pub limit: u64,
    pub token: Option<String>,
    pub slot: Option<Value>,
    pub block_time: Option<Value>,
}

const SLOT_OPS: [&str; 4] = ["gte", "gt", "lte", "lt"];
const TIME_OPS: [&str; 5] = ["gte", "gt", "lte", "lt", "eq"];

fn invalid(message: &str) -> Error {
    Error::new("INVALID_QUERY", message)
}

/// A provider page token: `slot:position`.
fn cursor(token: &str) -> Option<(u64, u64)> {
    let (slot, index) = token.split_once(':')?;
    Some((slot.parse().ok()?, index.parse().ok()?))
}

/// A comparison filter as the provider takes it: some of `ops`, each with an integer bound.
fn comparison(filter: &Option<Value>, ops: &[&str], what: &str) -> Result<Value, Error> {
    let Some(filter) = filter else {
        return Ok(Value::Null);
    };
    let bounds = filter
        .as_object()
        .filter(|bounds| !bounds.is_empty())
        .ok_or_else(|| invalid(what))?;
    for (op, bound) in bounds {
        if !ops.contains(&op.as_str()) || !(bound.is_i64() || bound.is_u64()) {
            return Err(invalid(what));
        }
    }
    Ok(filter.clone())
}

/// Whether `value` meets every bound of a validated comparison filter.
fn meets(value: i128, filter: &Value) -> bool {
    filter.as_object().is_none_or(|bounds| {
        bounds.iter().all(|(op, bound)| {
            let bound = bound
                .as_i64()
                .map(i128::from)
                .or(bound.as_u64().map(i128::from));
            bound.is_some_and(|bound| match op.as_str() {
                "gte" => value >= bound,
                "gt" => value > bound,
                "lte" => value <= bound,
                "lt" => value < bound,
                _ => value == bound,
            })
        })
    })
}

/// One page of an address's history before the position, from the provider's own index read
/// up to the boundary slot: entries in the boundary slot at or after the position never
/// happened on the fork, and those before it must be the boundary block's own transactions.
/// Every entry must meet the requested filters. Oldest first, reaching the position ends the
/// history.
pub(in super::super) fn address_history(
    session: &mut impl Archive,
    block: &Value,
    slot: u64,
    position: usize,
    query: AddressQuery,
) -> Result<Value, Error> {
    if base58(&json!(query.address), 32, "history address").is_err()
        || !(1..=1000).contains(&query.limit)
    {
        return Err(invalid(
            "address history needs an address and a limit of 1..=1000",
        ));
    }
    if query
        .token
        .as_deref()
        .is_some_and(|token| cursor(token).is_none())
    {
        return Err(invalid("address history page token"));
    }
    let slots = comparison(&query.slot, &SLOT_OPS, "address history slot filter")?;
    let times = comparison(&query.block_time, &TIME_OPS, "address history time filter")?;
    let page = session.archive(
        &json!({"kind":"transactionsForAddress","address":query.address,
        "sortOrder":query.order,"tokenAccounts":query.token_accounts,"status":query.status,
        "limit":query.limit,"paginationToken":query.token,"slot":slots,"blockTime":times,
        "untilSlot":slot}),
    )?;
    let boundary = transactions(block)?;
    let rows = page["data"]
        .as_array()
        .filter(|rows| rows.len() as u64 <= query.limit)
        .ok_or_else(|| integrity("provider history is missing or exceeds its limit"))?;
    // The request's cursor is exclusive: every row, and the next cursor, lie strictly past it
    // in the requested order.
    let beyond = |at: (u64, u64), past: (u64, u64)| match query.order {
        SortOrder::Asc => at > past,
        SortOrder::Desc => at < past,
    };
    let request = query.token.as_deref().and_then(cursor);
    let (mut data, mut previous, mut last, mut reached) = (Vec::new(), request, None, false);
    for entry in rows {
        let at = entry["slot"]
            .as_u64()
            .zip(entry["transactionIndex"].as_u64())
            .filter(|(at, _)| *at <= slot)
            .ok_or_else(|| integrity("provider history entry outside the fork's history"))?;
        let name = base58(&entry["signature"], 64, "provider history signature")?;
        let ordered = previous.is_none_or(|previous| beyond(at, previous));
        let status = match query.status {
            Status::Any => true,
            Status::Succeeded => entry["err"].is_null(),
            Status::Failed => !entry["err"].is_null(),
        };
        let time = times.is_null()
            || entry["blockTime"]
                .as_i64()
                .is_some_and(|time| meets(time.into(), &times));
        if !ordered || !status || !time || !meets(at.0.into(), &slots) {
            return Err(integrity("provider history does not meet its query"));
        }
        previous = Some(at);
        last = Some(at);
        if at.0 == slot {
            if at.1 >= position as u64 {
                reached = true;
                continue;
            }
            if signature(&boundary[at.1 as usize])? != name {
                return Err(integrity(
                    "provider history differs from the boundary block",
                ));
            }
        }
        data.push(entry.clone());
    }
    let next = match &page["paginationToken"] {
        Value::Null => Value::Null,
        Value::String(next) => {
            // A next cursor advances past the request and is not behind the page's last row.
            let advances = cursor(next).is_some_and(|at| {
                request.is_none_or(|request| beyond(at, request))
                    && last.is_none_or(|last| at == last || beyond(at, last))
            });
            match (advances, query.order, reached) {
                (false, ..) => {
                    return Err(integrity("provider history page token does not advance"))
                }
                (true, SortOrder::Asc, true) => Value::Null,
                (true, ..) => json!(next),
            }
        }
        _ => return Err(integrity("provider history page token")),
    };
    Ok(json!({"data":data,"paginationToken":next}))
}
