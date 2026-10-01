use super::*;

impl DebugController {
    /// Nonblocking frontend polling. `None` means no queued event; terminal
    /// failure and a disconnected producer are explicit errors.
    pub fn try_next_event(&self) -> Result<Option<Value>, Error> {
        if let Some(error) = self
            .failure
            .lock()
            .map_err(|e| error("DEBUG_CHANNEL", format!("terminal state poisoned: {e}")))?
            .as_ref()
        {
            return Err(error.clone());
        }
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(error(
                "DEBUG_DISCONNECTED",
                "debug event producer finished or disconnected",
            )),
        }
    }
    pub fn send(&self, command: DebugCommand) -> Result<(), Error> {
        require(
            command.phase.len() <= 64,
            "DEBUG_CONFIG",
            "phase label too long",
        )?;
        if let DebugAction::InspectAccount { pubkey, .. } = &command.action {
            require(pubkey.len() <= 44, "DEBUG_CONFIG", "account name too long")?;
        }
        if let DebugAction::SourceVariable { name } = &command.action {
            require(name.len() <= 128, "DEBUG_CONFIG", "variable name too long")?;
        }
        if let DebugAction::SourceBreakpoint { file, .. } = &command.action {
            require(file.len() <= 4096, "DEBUG_CONFIG", "source path too long")?;
        }
        self.commands.try_send(command).map_err(|e| {
            error(
                "DEBUG_CHANNEL",
                format!("command channel cannot accept bounded request: {e}"),
            )
        })
    }
    pub fn next_event(&self, budget: &ExecutionBudget) -> Result<Value, Error> {
        loop {
            if let Some(error) = self
                .failure
                .lock()
                .map_err(|e| error("DEBUG_CHANNEL", format!("terminal state poisoned: {e}")))?
                .as_ref()
            {
                return Err(error.clone());
            }
            budget.check().map_err(protocol_error)?;
            match self.events.recv_timeout(Duration::from_millis(5)) {
                Ok(event) => return Ok(event),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(error(
                        "DEBUG_DISCONNECTED",
                        "debug event producer finished or disconnected",
                    ))
                }
            }
        }
    }
}
