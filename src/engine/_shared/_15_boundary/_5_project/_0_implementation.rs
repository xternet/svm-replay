use super::*;
/// Read a bounded local projection request; no provider access.
pub fn project_file(path: &std::path::Path) -> Result<Value, Error> {
    let raw = crate::shared::runtime::read_bounded_file(path, 1024 * 1024)
        .map_err(|e| Error::new("REQUEST_IO", e.to_string()))?;
    project(&parse_json(&raw)?)
}
fn put(boundary: &mut Value, k: &str, state: &Option<Value>) -> Result<(), Error> {
    match state {
        None => boundary["absent"]
            .as_array_mut()
            .ok_or_else(|| invalid("absent missing"))?
            .push(json!(k)),
        Some(a) => {
            boundary["accounts"]
                .as_object_mut()
                .ok_or_else(|| invalid("accounts missing"))?
                .insert(k.to_owned(), a.clone());
        }
    }
    Ok(())
}
fn initial(job: &Job) -> Result<Value, Error> {
    let fixture = job.fixture()?;
    let target = job.target()?;
    let parent = _2_history::parents(job)?;
    let transaction = string(field(target, "transactionBase64")?)?;
    let (static_keys, extras) = _2_history::loaded(transaction, &parent)?;
    let touched = _2_history::touched(job)?;
    let begin = _2_history::snapshot(job, false)?;
    let used = _2_history::snapshot(job, true)?;
    let mut b = json!({"accounts":{},"absent":[],"unavailable":[]});
    for k in static_keys {
        put(
            &mut b,
            &k,
            begin
                .get(&k)
                .ok_or_else(|| invalid(format!("{k} missing from engine snapshot")))?,
        )?;
    }
    for k in extras {
        let image = parent.get(&k);
        let stale = touched.contains(&k) || SLOT_SYSVARS.contains(&k.as_str());
        if !stale
            && used.contains_key(&k)
            && image
                .and_then(Option::as_ref)
                .is_none_or(|a| a["owner"] != SYSVAR)
            && used.get(&k) != image
        {
            return Err(Error::new(
                "BOUNDARY_MISMATCH",
                format!("{k} differs from its parent-slot image at first use"),
            ));
        }
        let state = match used.get(&k) {
            Some(a) => Some(a),
            None if !stale => image,
            None => None,
        };
        match state {
            Some(a) => put(&mut b, &k, a)?,
            None => b["unavailable"]
                .as_array_mut()
                .ok_or_else(|| invalid("unavailable missing"))?
                .push(json!(k)),
        }
    }
    for (k, a) in &parent {
        if let Some(a) = a {
            if a["owner"] == SYSVAR
                && !SLOT_SYSVARS.contains(&k.as_str())
                && !_4_expansion::known(&b, k)?
            {
                put(&mut b, k, &Some(a.clone()))?;
            }
        }
    }
    let clock = image(field(fixture, "clock")?)?;
    let clock_data = bytes(field(&clock, "dataBase64")?)?;
    if clock_data.len() != 40 {
        return Err(invalid("Clock sysvar must be 40 bytes"));
    }
    let runtime = field(fixture, "runtime")?;
    let pin = array(field(runtime, "genericSysvars")?)?
        .iter()
        .find(|s| s["pubkey"] == clock["pubkey"])
        .ok_or_else(|| invalid("Clock runtime pin missing"))?;
    if pin["dataSha256"] != Digest::of(&clock_data).as_str() {
        return Err(invalid("Clock differs from verified runtime pin"));
    }
    put(
        &mut b,
        string(field(&clock, "pubkey")?)?,
        &Some(clock.clone()),
    )?;
    let original = field(&job.result, "original")?;
    for t in array(field(original, "accountTransitions")?)? {
        let k = string(field(t, "pubkey")?)?;
        if array(&b["unavailable"])?.iter().any(|v| v == k) {
            continue;
        }
        let before = field(t, "before")?;
        let account = b["accounts"].get(k);
        let same = if before["presence"] == "absent" {
            account.is_none()
        } else {
            match account {
                Some(a) => {
                    a["lamports"] == before["lamports"]
                        && a["owner"] == before["owner"]
                        && Digest::of(&bytes(field(a, "dataBase64")?)?).as_str()
                            == string(field(before, "dataHash")?)?
                }
                None => false,
            }
        };
        if !same {
            return Err(Error::new(
                "BOUNDARY_MISMATCH",
                format!("{k} differs from verified before-state"),
            ));
        }
    }
    let block = parse_json(&bytes(field(&job.prepared, "rawBlockBase64")?)?)?;
    let slot = number(field(target, "targetSlot")?)?;
    let features = _3_features::binding(field(field(runtime, "binding")?, "features")?, slot)?;
    let values = json!({"signature":field(target,"signature")?,"transactionBase64":transaction,"family":field(&job.prepared,"family")?,
        "parentSlot":field(target,"parentSlot")?,"targetSlot":slot,"blockHeight":number(field(field(&block,"result")?,"blockHeight")?)?,
        "epoch":u64::from_le_bytes(clock_data[16..24].try_into().map_err(|_| invalid("Clock epoch width"))?),"clock":clock,
        "features":features,"activeFeatures":features.iter().map(|f|f["id"].clone()).collect::<Vec<_>>(),
        "predecessors":array(field(target,"prefixSignatures")?)?.len(),"expected":{"original":original},"receiptPath":job.path,
        "evidence":[job.path],"runtime":job.runtime});
    b.as_object_mut()
        .ok_or_else(|| invalid("boundary missing"))?
        .extend(object(&values)?.clone());
    Ok(b)
}
fn pending(b: &Value, extra: &[String]) -> Result<Vec<String>, Error> {
    let mut keys = Vec::new();
    for k in extra.iter().cloned().chain(_4_expansion::companions(b)?) {
        key(&k)?;
        if !keys.contains(&k) && !_4_expansion::known(b, &k)? {
            keys.push(k);
        }
    }
    Ok(keys)
}
/// Rebuild a fork projection solely from pinned jobs. Returns the next native-planned widening, if needed.
pub fn project(input: &Value) -> Result<Value, Error> {
    if input["schema"] != "svm-replay-boundary-request/v1" {
        return Err(invalid("unknown boundary request schema"));
    }
    let jobs = array(field(input, "jobs")?)?;
    if jobs.is_empty() || jobs.len() > 5 {
        return Err(invalid("boundary needs 1..5 jobs"));
    }
    let extra = array(field(input, "extra")?)?
        .iter()
        .map(|v| Ok(string(v)?.to_owned()))
        .collect::<Result<Vec<_>, Error>>()?;
    let first = Job::load(&jobs[0])?;
    let mut b = initial(&first)?;
    let mut needed = pending(&b, &extra)?;
    for descriptor in &jobs[1..] {
        if needed.is_empty() {
            return Err(invalid("unexpected widening job"));
        }
        let job = Job::load(descriptor)?;
        for k in ["signature", "parentSlot", "targetSlot"] {
            if field(first.target()?, k)? != field(job.target()?, k)? {
                return Err(invalid("widening boundary identity differs"));
            }
        }
        let wanted = _4_expansion::footprint(string(&b["transactionBase64"])?, &needed)?;
        if job.target()?["replacementTransactionBase64"] != wanted {
            return Err(invalid("widening transaction differs from native plan"));
        }
        _4_expansion::widen(&mut b, &job, &needed)?;
        needed = pending(&b, &extra)?;
    }
    if !needed.is_empty() && jobs.len() == 5 {
        return Err(Error::new(
            "BOUNDARY_INCOMPLETE",
            "companion accounts kept growing after 4 rounds",
        ));
    }
    match _3_features::derive(array(&b["features"])?, object(&b["accounts"])?) {
        Ok(images) => {
            b["featureImages"] = images;
            b["featureError"] = Value::Null;
        }
        Err(e) => {
            b["featureImages"] = json!({});
            b["featureError"] = json!(e);
        }
    }
    let next = if needed.is_empty() {
        Value::Null
    } else {
        json!({"transactionBase64":_4_expansion::footprint(string(&b["transactionBase64"])?,&needed)?,"accounts":needed})
    };
    Ok(json!({"schema":"svm-replay-boundary/v1","outcome":"COMPLETED","boundary":b,"next":next}))
}
