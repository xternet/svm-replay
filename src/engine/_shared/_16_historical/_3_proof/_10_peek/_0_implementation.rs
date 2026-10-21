use super::*;

impl VerifiedSession {
    /// What the active fork already holds for `pubkeys`, without acquiring anything: each
    /// entry is `{"loaded":false}` for an account never loaded (its historical image is
    /// unknown, not absent), or `{"loaded":true,"account":image-or-null}`.
    pub fn peek_accounts(
        &mut self,
        pubkeys: Vec<String>,
        budget: &ExecutionBudget,
    ) -> Result<Value, Error> {
        let missing: Vec<String> = match self.native.request(
            SessionCommand::Accounts {
                pubkeys: pubkeys.clone(),
            },
            budget,
        ) {
            Ok(images) => return loaded(&pubkeys, &[], images),
            Err(error) if error.code == "NEEDS_INPUT" => error
                .details
                .as_ref()
                .filter(|detail| detail["cause"]["kind"] == "account")
                .and_then(|detail| detail["cause"]["pubkeys"].as_array())
                .ok_or_else(|| Error::new("SESSION_PROTOCOL", "missing account list absent"))?
                .iter()
                .map(|key| key.as_str().map(str::to_owned))
                .collect::<Option<_>>()
                .ok_or_else(|| Error::new("SESSION_PROTOCOL", "invalid missing account"))?,
            Err(error) => return Err(error),
        };
        let known: Vec<String> = pubkeys
            .iter()
            .filter(|key| !missing.contains(key))
            .cloned()
            .collect();
        let images = match known.is_empty() {
            true => json!([]),
            false => self
                .native
                .request(SessionCommand::Accounts { pubkeys: known }, budget)?,
        };
        loaded(&pubkeys, &missing, images)
    }
}

/// Pairs each requested key with its image in request order; `missing` keys were never loaded.
fn loaded(pubkeys: &[String], missing: &[String], images: Value) -> Result<Value, Error> {
    let mut images = images
        .as_array()
        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "account images missing"))?
        .iter();
    let entries = pubkeys
        .iter()
        .map(|key| match missing.contains(key) {
            true => Ok(json!({"loaded": false})),
            false => images
                .next()
                .map(|image| json!({"loaded": true, "account": image}))
                .ok_or_else(|| Error::new("SESSION_PROTOCOL", "account images missing")),
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if images.next().is_some() {
        return Err(Error::new("SESSION_PROTOCOL", "unrequested account image"));
    }
    Ok(Value::Array(entries))
}
