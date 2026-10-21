use super::*;

impl NativeSession {
    /// Observe idle worker failure too, rather than waiting for the next command.
    pub fn check(&mut self) -> Result<(), Error> {
        if let Err(error) = self.budget.check() {
            return Err(self.abort(protocol_error(error)));
        }
        match self.supervisor.as_ref() {
            Some(worker) if !worker.is_finished() => Ok(()),
            Some(_) => Err(self.abort(error("native session exited unexpectedly"))),
            None => Err(error("native session is closed")),
        }
    }

    pub fn ready(&self) -> &Value {
        &self.ready
    }

    pub fn request(
        &mut self,
        command: SessionCommand,
        call: &ExecutionBudget,
    ) -> Result<Value, Error> {
        let envelope = (|| {
            let id = self.next_id;
            let request = serde_json::json!({"schema":"svm-native-session-command/v1","id":id,"command":command});
            let wire = self
                .wire
                .as_mut()
                .ok_or_else(|| error("session is closed"))?;
            wire.write(&request, &self.budget, call)?;
            let reply = wire.read(&self.budget, call)?;
            if reply["schema"] != "svm-native-session/v1"
                || reply["id"].as_u64() != Some(id)
                || reply.get("result").is_some() == reply.get("error").is_some()
            {
                return Err(error("invalid or out-of-order native session response"));
            }
            self.next_id = id
                .checked_add(1)
                .ok_or_else(|| error("session sequence exhausted"))?;
            Ok(reply)
        })();
        let reply = match envelope {
            Ok(reply) => reply,
            Err(primary) => return Err(self.abort(primary)),
        };
        if let Some(remote) = reply.get("error") {
            let code = remote["code"]
                .as_str()
                .ok_or_else(|| error("session error code missing"));
            let message = remote["message"]
                .as_str()
                .ok_or_else(|| error("session error message missing"));
            return match (code, message) {
                (Ok(code), Ok(message)) => {
                    Err(Error::new(code, message).with_details(remote.clone()))
                }
                _ => Err(self.abort(error("malformed native session error"))),
            };
        }
        Ok(reply["result"].clone())
    }

    /// Close, await the pinned process owner and remove all owned scratch/IPC.
    pub fn close(mut self) -> Result<WorkerOutput, Error> {
        let call = self.budget.clone();
        let reply = self.request(SessionCommand::Close, &call)?;
        if reply["closed"] != true {
            return Err(self.abort(error("invalid close acknowledgment")));
        }
        let output = self.finish()?;
        if output.value["schema"] != "svm-native-session-terminal/v1"
            || output.value["closed"] != true
        {
            return Err(error("invalid native session terminal artifact"));
        }
        Ok(output)
    }

    fn finish(&mut self) -> Result<WorkerOutput, Error> {
        self.wire.take();
        let supervisor = self
            .supervisor
            .take()
            .ok_or_else(|| error("session supervisor already joined"))?;
        let result = match supervisor.join() {
            Ok(result) => result.map_err(protocol_error),
            Err(_) => Err(error("native session supervisor panicked")),
        };
        let cleanup = match self.ipc.take() {
            Some(ipc) => ipc
                .close()
                .map_err(|e| error(format!("remove private session IPC: {e}"))),
            None => Ok(()),
        };
        match (result, cleanup) {
            (result, Ok(())) => result,
            (Err(primary), Err(cleanup)) => {
                Err(cleanup.with_details(serde_json::json!({"precedingFailure":primary})))
            }
            (Ok(_), Err(cleanup)) => Err(cleanup),
        }
    }

    pub(in super::super) fn abort(&mut self, primary: Error) -> Error {
        self.budget.cancel();
        if self.supervisor.is_none() {
            return primary;
        }
        match self.finish() {
            Ok(_) => primary,
            Err(cleanup) => primary.with_details(serde_json::json!({"supervisor":cleanup})),
        }
    }
}

impl Drop for NativeSession {
    fn drop(&mut self) {
        if self.supervisor.is_some() {
            self.budget.cancel();
            if let Err(error) = self.finish() {
                eprintln!("native session drop cleanup: {error}");
            }
        }
    }
}
