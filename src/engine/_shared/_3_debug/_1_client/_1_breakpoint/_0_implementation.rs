use super::*;

impl DebugClient {
    pub fn breakpoint(&mut self, pc: u64, enabled: bool) -> Result<(), Error> {
        let response = self.request(
            &format!("{}0,{pc:x},0", if enabled { 'Z' } else { 'z' }),
            PendingKind::Text,
        )?;
        require(
            response == "OK",
            "DEBUG_TARGET",
            "software breakpoint unsupported",
        )?;
        if enabled {
            self.breakpoints.insert(pc);
        } else {
            self.breakpoints.remove(&pc);
        }
        Ok(())
    }
}
impl DebugClient {
    pub(crate) fn has_breakpoint(&self, pc: u64) -> bool {
        self.breakpoints.contains(&pc)
    }
}
impl DebugClient {
    pub(in super::super) fn advance(&mut self, command: char) -> Result<(), Error> {
        self.stopped()?;
        self.begin(
            &command.to_string(),
            PendingKind::Advance {
                command,
                drain_delayed: command == 'c' && self.delayed_interrupt,
            },
        )?;
        self.state = DebugState::Running;
        Ok(())
    }
}
impl DebugClient {
    pub fn step(&mut self) -> Result<(), Error> {
        self.advance('s')
    }
}
impl DebugClient {
    pub fn resume(&mut self) -> Result<(), Error> {
        self.advance('c')
    }
}
impl DebugClient {
    pub fn poll_stop(&mut self) -> Result<Option<Stop>, Error> {
        match self.poll()? {
            Some(Reply::Stop(stop)) => Ok(Some(stop)),
            Some(Reply::Text(_)) => Err(error("DEBUG_PROTOCOL", "non-stop pending response")),
            None => Ok(None),
        }
    }
}
impl DebugClient {
    pub fn wait_stop(&mut self) -> Result<Stop, Error> {
        require(
            self.state == DebugState::Running,
            "DEBUG_STATE",
            "no active execution command",
        )?;
        loop {
            if let Some(stop) = self.poll_stop()? {
                return Ok(stop);
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl DebugClient {
    pub fn pause(&mut self) -> Result<(), Error> {
        require(
            self.state == DebugState::Running && self.pending.is_some() && !self.pause_pending,
            "DEBUG_STATE",
            "pause requires a running command without outstanding interrupt",
        )?;
        self.write(&[3])?;
        self.pause_pending = true;
        Ok(())
    }
}
impl DebugClient {
    pub fn detach(&mut self) -> Result<(), Error> {
        let response = self.request("D", PendingKind::Text)?;
        require(
            response == "OK",
            "DEBUG_PROTOCOL",
            "detach not acknowledged",
        )?;
        self.close()
    }
}
impl DebugClient {
    pub fn close(&mut self) -> Result<(), Error> {
        if matches!(self.state, DebugState::Running | DebugState::Stopped) {
            self.state = DebugState::Cancelled;
        }
        self.pending = None;
        match self.stream.shutdown(Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotConnected => Ok(()),
            Err(e) => Err(io_error("close debugger socket", e)),
        }
    }
}
