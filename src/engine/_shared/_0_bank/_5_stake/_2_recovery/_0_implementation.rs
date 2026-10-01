use super::*;

pub(in super::super) fn recover(input: &Value) -> Result<Value> {
    let id = pubkey(field(input, "pubkey")?)?;
    let boundary = field(input, "boundary")?;
    let slot = integer(field(boundary, "slot")?)?;
    let parent_slot = integer(field(boundary, "parentSlot")?)?;
    let height = integer(field(boundary, "blockHeight")?)?;
    let count = integer(field(boundary, "transactionCount")?)?;
    pubkey(field(boundary, "blockhash")?)?;
    check(
        parent_slot < slot && height > 0,
        "actual parent order/height",
    )?;
    let runtime = field(input, "runtime")?;
    let executor = string(field(runtime, "executorSourceId")?)?;
    let (_, version, commit) = ERAS
        .iter()
        .find(|(name, _, _)| *name == executor)
        .ok_or_else(|| fail("unreviewed Bank era"))?;
    let features = array(field(runtime, "activeExecutionFeatureIds")?)?;
    for value in features {
        string(value)?;
    }
    check(
        !features
            .iter()
            .any(|v| v == "a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"),
        "unreviewed Bank era",
    )?;
    let adjustment = if *version == "4.2.1" && features.iter().any(|v| v == ADJUST_RENT) {
        Some(rent_phase(input.get("phaseContext"), slot)?)
    } else {
        None
    };
    check(
        id != "1nc1nerator11111111111111111111111111111111",
        "later Bank write target",
    )?;
    let block_source = field(input, "block")?;
    let parent_source = field(input, "parentBlock")?;
    let block = receipt(block_source, "getBlock")?;
    let parent = receipt(parent_source, "getBlock")?;
    let params = array(field(field(block_source, "request")?, "params")?)?;
    let options = &params[1];
    check(
        params[0] == slot
            && field(options, "commitment")? == "finalized"
            && field(options, "encoding")? == "json"
            && field(options, "transactionDetails")? == "full"
            && field(options, "rewards")? == &json!(true)
            && matches!(
                field(options, "maxSupportedTransactionVersion")?.as_u64(),
                Some(0 | 1)
            ),
        "complete full block required",
    )?;
    let params = array(field(field(parent_source, "request")?, "params")?)?;
    check(
        params[0] == parent_slot && field(&params[1], "commitment")? == "finalized",
        "parent request identity",
    )?;
    check(
        field(&block, "parentSlot")? == &json!(parent_slot)
            && field(&block, "blockhash")? == field(boundary, "blockhash")?
            && field(&block, "blockHeight")? == &json!(height)
            && field(&parent, "blockHeight")? == &json!(height - 1)
            && field(&parent, "blockhash")? == field(&block, "previousBlockhash")?,
        "canonical parent/block identity",
    )?;
    let transactions = array(field(&block, "transactions")?)?;
    check(
        transactions.len() as u64 == count,
        "incomplete transaction count",
    )?;
    let accesses = nonmutating_accesses(transactions, id)?;
    let rewards = array(field(&block, "rewards").map_err(|_| fail("complete rewards missing"))?)?;
    let mut matches = Vec::new();
    for reward in rewards {
        object(reward)?;
        if reward.get("pubkey") == Some(&json!(id)) {
            matches.push(reward);
        }
    }
    check(
        matches.is_empty()
            || (matches.len() == 1
                && (field(matches[0], "rewardType")? == "Staking"
                    || *version == "4.2.1"
                        && field(matches[0], "rewardType")? == "DeactivatedStake")),
        "absent or unique Staking reward required",
    )?;
    let reward = if matches.is_empty() {
        0
    } else {
        let value = uint(field(matches[0], "lamports")?)?;
        check(value > 0, "positive reward")?;
        value
    };
    let before = stake_account(field(input, "parentAccount")?, id, parent_slot)?;
    let account = stake_account(field(input, "targetAccount")?, id, slot)?;
    check(
        field(&before, "rentEpoch")? == field(&account, "rentEpoch")?,
        "unexpected rent change",
    )?;
    let legacy = *version == "2.3.9";
    if legacy {
        check(
            field(&before, "rentEpoch")? == &json!(u64::MAX.to_string()),
            "unproven eager-rent phase: parent and target must already carry exempt marker",
        )?;
    }
    let lamports = uint(field(&account, "lamports")?)?;
    check(
        lamports.checked_sub(uint(field(&before, "lamports")?)?) == Some(reward),
        "reward lamport arithmetic",
    )?;
    if reward > 0 {
        check(
            lamports == uint(field(matches[0], "postBalance")?)?,
            "reward post-balance",
        )?;
    }
    let old = bytes(field(&before, "dataBase64")?)?;
    let new = bytes(field(&account, "dataBase64")?)?;
    let sum = u128::from(read_u64(&old, 156)?) + u128::from(reward);
    let delegated = sum.min(u128::from(u64::MAX)) as u64;
    let available = match &adjustment {
        None => lamports,
        Some(value) => lamports.saturating_sub(value.minimum),
    };
    let unchanged = reward == 0 && old == new;
    let expected = if adjustment.is_none() || unchanged {
        sum
    } else {
        u128::from(delegated.min(available))
    };
    check(
        u128::from(read_u64(&new, 156)?) == expected,
        "delegation reward arithmetic",
    )?;
    let deactivation = match &adjustment {
        Some(value) if !unchanged && expected == 0 && expected != u128::from(delegated) => {
            value.rewarded_epoch
        }
        _ => read_u64(&old, 172)?,
    };
    check(
        read_u64(&new, 172)? == deactivation,
        "delegation deactivation epoch",
    )?;
    if reward > 0 {
        check(
            read_u64(&new, 188)? > read_u64(&old, 188)?,
            "reward credits must advance",
        )?;
    }
    for (start, end) in [(0, 156), (164, 172), (180, 188), (196, 200)] {
        check(
            new[start..end] == old[start..end],
            "unrelated stake bytes changed",
        )?;
    }
    let nonmutating = !accesses.is_empty();
    let positive = reward > 0;
    let policy = if *version == "4.0.0" {
        match (nonmutating, positive) {
            (false, true) => "agave-4.0.0-tower-positive-staking-reward/v1".into(),
            (false, false) => "agave-4.0.0-tower-untouched-zero-staking-reward/v1".into(),
            (true, true) => "agave-4.0.0-tower-nonmutating-positive-staking-reward/v1".into(),
            (true, false) => "agave-4.0.0-tower-nonmutating-zero-staking-reward/v1".into(),
        }
    } else {
        let prefix = if legacy {
            "exempt-"
        } else if *version == "4.2.1" {
            if adjustment.is_some() {
                "rent-adjusted-delegation-"
            } else {
                "unadjusted-delegation-"
            }
        } else {
            ""
        };
        format!(
            "agave-{version}-tower-{prefix}{}-{}-staking-reward/v1",
            if nonmutating {
                "nonmutating"
            } else {
                "untouched"
            },
            if positive { "positive" } else { "zero" }
        )
    };
    let mut proof = json!({"schema":if nonmutating{"svm-simulate-nonmutating-stake-initialization/v1"}else{"svm-simulate-untouched-stake-initialization/v1"},
        "phasePolicy":policy,"phaseSourceCommit":commit,"slot":slot,"parentSlot":parent_slot,"blockhash":field(boundary,"blockhash")?,
        "blockHeight":height,"transactionCount":transactions.len(),"pubkey":id,"executorSourceId":executor,"rewardLamports":reward.to_string(),
        "blockResponseSha256":field(block_source,"responseSha256")?,"parentBlockResponseSha256":field(parent_source,"responseSha256")?,
        "parentAccountResponseSha256":field(field(input,"parentAccount")?,"responseSha256")?,
        "targetAccountResponseSha256":field(field(input,"targetAccount")?,"responseSha256")?,
        "parentDataSha256":hash(old),"targetDataSha256":hash(new)});
    if nonmutating {
        proof["transactionAccessProof"] = json!(accesses);
    }
    if legacy {
        proof["rentCollectionProof"] = json!("parent-and-target-exempt-marker/v1");
    }
    if *version == "4.2.1" {
        proof["rewardType"] = if matches.is_empty() {
            json!("None")
        } else {
            field(matches[0], "rewardType")?.clone()
        };
    }
    if let Some(adjustment) = adjustment {
        proof["rentAdjustmentProof"] = adjustment.proof;
    }
    Ok(json!({"account":account,"parentAccount":before,"proof":proof}))
}
