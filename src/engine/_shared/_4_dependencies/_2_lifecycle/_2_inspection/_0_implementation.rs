use super::*;

pub fn assert_inspected_loader_lifecycle(
    raw: &Value,
    index: u64,
    context: &HistoricalLoaderContext,
) -> Result<()> {
    safe_integer(context.parent_slot, "loader parent slot")?;
    let mut accounts = BTreeMap::new();
    for a in &context.accounts {
        let key = a["pubkey"]
            .as_str()
            .ok_or_else(|| invalid("loader parent pubkey missing"))?;
        check(
            accounts.insert(key, a).is_none(),
            "duplicate loader parent account input",
        )?;
    }
    let instructions: Vec<_> = resolve(raw, index)?
        .into_iter()
        .filter(|i| i.program == LOADER || i.program == LOADER_V4)
        .collect();
    check(
        !instructions.is_empty(),
        "invoked loader has no raw instruction evidence",
    )?;
    for i in &instructions {
        check(i.program == LOADER, "loader-v4 lifecycle is not supported")?;
        let state = tag(&i.data).ok_or_else(|| invalid("loader instruction tag missing"))?;
        let a = &i.accounts;
        let slot = context.parent_slot;
        match state {
            0 => {
                check(
                    i.data.len() == 4 && a.len() >= 2,
                    "malformed InitializeBuffer",
                )?;
                if exact(&a[0], &accounts, slot)?["presence"] == "present" {
                    parent(&a[0], false, &accounts, slot)?;
                }
            }
            1 => {
                check(
                    i.data.len() >= 16
                        && (i.data.len() - 16) as u64 == uint(&i.data, 8)?
                        && a.len() >= 2,
                    "malformed Buffer Write",
                )?;
                buffer(&a[0], raw, index, i, context, &accounts)?;
            }
            2 => {
                check(deploy(i), "malformed Deploy")?;
                exact(&a[1], &accounts, slot)?;
                exact(&a[2], &accounts, slot)?;
                buffer(&a[3], raw, index, i, context, &accounts)?;
            }
            3 | 6 => {
                check(
                    i.data.len() == if state == 3 { 4 } else { 8 }
                        && a.len() >= if state == 3 { 7 } else { 2 },
                    if state == 3 {
                        "malformed Upgrade"
                    } else {
                        "malformed ExtendProgram"
                    },
                )?;
                let p = exact(&a[1], &accounts, slot)?;
                let pd = exact(&a[0], &accounts, slot)?;
                if p["presence"] == "absent" || pd["presence"] == "absent" {
                    earlier_deploy(raw, index, i, context, &a[1], &a[0])?;
                    if state == 3 {
                        buffer(&a[2], raw, index, i, context, &accounts)?;
                    }
                    continue;
                }
                mapping(
                    &a[1],
                    &a[0],
                    &accounts,
                    slot,
                    &format!("{}: parent Program -> ProgramData mapping mismatch", a[1]),
                )?;
                let data = parent(&a[0], false, &accounts, slot)?;
                authority(&a[0], &data, 3)?;
                if state == 3 {
                    buffer(&a[2], raw, index, i, context, &accounts)?;
                }
                check(
                    uint(&data, 4)? <= slot,
                    format!("{}: parent ProgramData deployment is in the future", a[0]),
                )?;
            }
            4 | 7 => {
                check(
                    i.data.len() == 4 && a.len() >= if state == 4 { 2 } else { 3 },
                    "malformed SetAuthority",
                )?;
                if exact(&a[0], &accounts, slot)?["presence"] == "present" {
                    let data = parent(&a[0], false, &accounts, slot)?;
                    if tag(&data) == Some(3) {
                        authority(&a[0], &data, 3)?;
                        check(
                            uint(&data, 4)? <= slot,
                            "authority parent deployment is in the future",
                        )?;
                        continue;
                    }
                } else if earlier(raw, index, i, context, |prior| {
                    if deploy(prior) && prior.accounts[1] == a[0] {
                        exact(&prior.accounts[2], &accounts, slot)?;
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                })? {
                    continue;
                }
                buffer(&a[0], raw, index, i, context, &accounts)?;
            }
            5 => {
                check(i.data.len() == 4 && a.len() >= 2, "malformed Close")?;
                if exact(&a[0], &accounts, slot)?["presence"] == "absent" {
                    if initialized(&a[0], raw, index, i, context)? {
                        continue;
                    }
                    check(a.len() >= 4, "absent ProgramData Close Program missing")?;
                    exact(&a[3], &accounts, slot)?;
                    earlier_deploy(raw, index, i, context, &a[3], &a[0])?;
                    continue;
                }
                let data = parent(&a[0], false, &accounts, slot)?;
                match tag(&data).ok_or_else(|| invalid("Close parent loader state missing"))? {
                    1 => {
                        check(a.len() >= 3, "Buffer Close authority missing")?;
                        authority(&a[0], &data, 1)?;
                    }
                    3 => {
                        check(a.len() >= 4, "ProgramData Close Program missing")?;
                        authority(&a[0], &data, 3)?;
                        mapping(
                            &a[3],
                            &a[0],
                            &accounts,
                            slot,
                            "Close parent Program -> ProgramData mapping mismatch",
                        )?;
                        check(
                            uint(&data, 4)? <= slot,
                            "Close parent deployment is in the future",
                        )?;
                    }
                    0 => {}
                    _ => return Err(invalid("uninspected Close parent loader state")),
                }
            }
            _ => {
                return Err(invalid(format!(
                    "uninspected loader-v3 lifecycle tag {state}"
                )))
            }
        }
    }
    Ok(())
}
