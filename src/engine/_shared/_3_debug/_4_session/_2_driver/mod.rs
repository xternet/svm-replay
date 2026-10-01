use super::*;

impl DebugDriver {
    /// Installs caller-admitted exact ELF data, never a path learned from runtime/DWARF.
    pub fn install_symbols(&mut self, symbols: ExactSymbols) -> Result<(), Error> {
        require(
            self.symbols.len() < 8,
            "SYMBOL_LIMIT",
            "session exact-ELF symbol limit",
        )?;
        let key = symbols.elf_sha256().as_str().to_owned();
        require(
            !self.symbols.contains_key(&key),
            "SYMBOL_IDENTITY",
            "ELF symbols already installed",
        )?;
        self.symbols.insert(key, symbols);
        Ok(())
    }
    pub fn symbol_admissions(&self) -> Vec<Value> {
        self.symbols.values().map(ExactSymbols::admission).collect()
    }
    /// Separate terminal state remains deliverable even when the event queue is full.
    pub fn failed(&mut self, error: &Error) -> Result<(), Error> {
        let mut failure = self.failure.lock().map_err(|e| {
            super::super::error("DEBUG_CHANNEL", format!("terminal state poisoned: {e}"))
        })?;
        if failure.is_none() {
            *failure = Some(error.clone());
        }
        Ok(())
    }
    pub(super) fn emit(&mut self, event: Value, transcript: &mut Vec<Value>) -> Result<(), Error> {
        self.event_count += 1;
        self.event_bytes = self
            .event_bytes
            .checked_add(canonical_json(&event).len())
            .ok_or_else(|| error("DEBUG_LIMIT", "event bytes overflow"))?;
        require(
            self.event_count <= 32768 && self.event_bytes <= 16 * 1024 * 1024,
            "DEBUG_LIMIT",
            "whole-session event count/byte limit exhausted",
        )?;
        self.events.try_send(event.clone()).map_err(|e| {
            error(
                "DEBUG_CHANNEL",
                format!("event consumer cannot accept bounded response: {e}"),
            )
        })?;
        transcript.push(event);
        Ok(())
    }
    pub fn complete(&mut self, receipt: &Value) -> Result<(), Error> {
        self.emit(
            json!({"kind":"session-complete","receipt":receipt}),
            &mut Vec::new(),
        )
    }
    pub fn phase_complete(&mut self, phase: &str, executions: &[u16]) -> Result<(), Error> {
        self.emit(json!({"kind":"transaction-complete","phase":phase,"executionIndices":executions,"executionMode":"interpreter-debug","scope":"finalized whole-worker output and verified mode parity; no inferred journal/ordinal mapping"}),&mut Vec::new())
    }
}
