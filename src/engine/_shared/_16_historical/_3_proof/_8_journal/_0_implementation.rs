use super::*;

fn journal(message: &str) -> Error {
    Error::new("JOURNAL_INTEGRITY", message)
}

impl VerifiedSession {
    /// The Hydrate for `keys`. During a journal replay it is the recorded one, checked
    /// against this exact need, this state and its retained receipt; otherwise a fresh
    /// verified expansion, recorded while a journal records.
    pub(in super::super) fn next_hydration(
        &mut self,
        keys: &[String],
        state: &Value,
        budget: &ExecutionBudget,
    ) -> Result<SessionCommand, Error> {
        let hydration = self
            .hydration
            .as_mut()
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "session keeps no hydration"))?;
        if self.replaying {
            let recorded = self
                .side_replay
                .pop_front()
                .ok_or_else(|| journal("the journal records no hydration for this read"))?;
            let command: SessionCommand = serde_json::from_value(recorded["command"].clone())
                .map_err(|e| journal(&format!("recorded Hydrate: {e}")))?;
            let SessionCommand::Hydrate {
                state_sha256,
                receipt_sha256,
                ..
            } = &command
            else {
                return Err(journal("a recorded side command is not a Hydrate"));
            };
            if recorded["keys"] != json!(keys)
                || state["stateSha256"].as_str() != Some(state_sha256.as_str())
            {
                return Err(journal("a recorded Hydrate answers another read or state"));
            }
            let path = recorded["receipt"]["path"]
                .as_str()
                .ok_or_else(|| journal("recorded receipt path missing"))?;
            let retained = crate::shared::runtime::file_sha256(Path::new(path))
                .map_err(|_| journal("the retained hydration receipt is missing"))?;
            if recorded["receipt"]["sha256"] != json!(receipt_sha256) || retained != *receipt_sha256
            {
                return Err(journal("the retained hydration receipt changed"));
            }
            let accounts = serde_json::from_value(recorded["extraAccounts"].clone())
                .map_err(|_| journal("recorded account scope malformed"))?;
            hydration.restore_extra_accounts(accounts);
            return Ok(command);
        }
        let command = hydration.expand(keys, &self.input, &self.pins, state, budget)?;
        if let Some(log) = &mut self.side_log {
            let (path, sha256) = hydration
                .last_receipt()
                .ok_or_else(|| Error::new("SESSION_PROTOCOL", "expansion receipt missing"))?;
            log.push(json!({"kind":"hydrate","keys":keys,"command":command,
                "extraAccounts":hydration.extra_accounts(),"receipt":{"path":path,"sha256":sha256}}));
        }
        Ok(command)
    }
    /// The retained inputs this session was opened from (its initial receipt and worker).
    pub fn input(&self) -> &SessionInput {
        &self.input
    }
    /// Starts recording the verified Hydrates this session sends, for a journal.
    pub fn record_side_commands(&mut self) {
        self.side_log.get_or_insert_with(Vec::new);
    }
    /// The Hydrates recorded since the last call.
    pub fn take_side_commands(&mut self) -> Vec<Value> {
        self.side_log
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
    /// Hands a journal record's Hydrates to the operation being replayed.
    pub fn replay_side_commands(&mut self, sides: Vec<Value>) {
        self.replaying = true;
        self.side_replay = sides.into();
    }
    /// Ends one replayed operation: it must have used exactly the Hydrates it recorded.
    pub fn finish_replay(&mut self) -> Result<(), Error> {
        self.replaying = false;
        if !self.side_replay.is_empty() {
            self.side_replay.clear();
            return Err(journal(
                "the journal records hydrations its operation no longer needs",
            ));
        }
        Ok(())
    }
    /// The active branch's native state digest.
    pub fn state_digest(&mut self, budget: &ExecutionBudget) -> Result<String, Error> {
        let state = self.native.request(SessionCommand::State, budget)?;
        state["stateSha256"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "native state digest missing"))
    }
}
