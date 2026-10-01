use super::*;

pub(in super::super) fn budget(
    tx: &Transaction<'_>,
    action: &str,
    id: &str,
    limits: Option<Counts>,
    amount: Option<Counts>,
) -> Result<Value> {
    match action {
        "open" if limits.is_some() && amount.is_none() => {
            limits
                .ok_or_else(|| StoreError::new("INVALID_REQUEST", "limits required"))?
                .validate()?;
        }
        "charge" | "observe" if amount.is_some() && limits.is_none() => {
            amount
                .ok_or_else(|| StoreError::new("INVALID_REQUEST", "amount required"))?
                .validate()?;
        }
        "inspect" if limits.is_none() && amount.is_none() => {}
        _ => {
            return Err(StoreError::new(
                "INVALID_REQUEST",
                "budget requires open+limits, charge/observe+amount, or inspect without either",
            ))
        }
    }
    let prior: Option<(Counts,Counts)> = tx.query_row("SELECT limit_reads,limit_requests,limit_bytes,used_reads,used_requests,used_bytes FROM budgets WHERE id=?1", [id], |r| Ok((Counts{reads:r.get(0)?,requests:r.get(1)?,bytes:r.get(2)?},Counts{reads:r.get(3)?,requests:r.get(4)?,bytes:r.get(5)?}))).optional()?;
    let (limit, mut used) = match prior {
        Some((limit, used)) => {
            if action == "open" && limits != Some(limit) {
                return Err(StoreError::new(
                    "BUDGET_CONFLICT",
                    "existing budget limits cannot change",
                ));
            }
            (limit, used)
        }
        None => {
            if action != "open" {
                return Err(StoreError::new("NOT_FOUND", "budget has not been opened"));
            }
            let limit =
                limits.ok_or_else(|| StoreError::new("INVALID_REQUEST", "limits required"))?;
            tx.execute(
                "INSERT INTO budgets VALUES(?1,?2,?3,?4,0,0,0)",
                params![
                    id,
                    limit.reads as i64,
                    limit.requests as i64,
                    limit.bytes as i64
                ],
            )?;
            (
                limit,
                Counts {
                    reads: 0,
                    requests: 0,
                    bytes: 0,
                },
            )
        }
    };
    if let Some(amount) = amount {
        let next = used.add(amount)?;
        if action == "charge" && next.exceeds(limit) {
            return Err(StoreError::new(
                "BUDGET_EXCEEDED",
                format!(
                    "reservation rejected; current={}, requested={}, limits={}",
                    used.value(),
                    amount.value(),
                    limit.value()
                ),
            ));
        }
        tx.execute(
            "UPDATE budgets SET used_reads=?1,used_requests=?2,used_bytes=?3 WHERE id=?4",
            params![
                next.reads as i64,
                next.requests as i64,
                next.bytes as i64,
                id
            ],
        )?;
        used = next;
    }
    if action == "observe" && used.exceeds(limit) {
        let mut error = StoreError::new(
            "BUDGET_EXCEEDED",
            "observed spend persisted and exceeds limits",
        );
        error.details = Some(json!({"used":used.value(),"limits":limit.value()}));
        return Ok(error.response());
    }
    Ok(json!({"version":1,"status":"OK","limits":limit.value(),"used":used.value()}))
}
