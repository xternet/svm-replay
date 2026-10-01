use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceNavigationKind {
    Next,
    Finish,
}

pub struct SourceNavigation {
    pub(in super::super) kind: SourceNavigationKind,
    pub(in super::super) depth: u64,
    pub(in super::super) location: SourceLocation,
    pub(in super::super) steps: usize,
    pub(in super::super) started: Instant,
}

impl SourceNavigation {
    pub(in super::super) fn at(
        kind: SourceNavigationKind,
        depth: u64,
        location: SourceLocation,
    ) -> Self {
        Self {
            kind,
            depth,
            location,
            steps: 0,
            started: Instant::now(),
        }
    }
    /// Admission observes a stopped VM but never starts execution. The caller sends each real step.
    pub fn begin(
        symbols: &ExactSymbols,
        client: &mut DebugClient,
        kind: SourceNavigationKind,
    ) -> Result<Self, Error> {
        let (depth, location) = Self::snapshot(symbols, client)?;
        require(
            location.file.is_some() && location.line.is_some(),
            "CAPABILITY_UNAVAILABLE",
            "current PC has no source line",
        )?;
        Ok(Self::at(kind, depth, location))
    }
    pub(in super::super) fn snapshot(
        symbols: &ExactSymbols,
        client: &mut DebugClient,
    ) -> Result<(u64, SourceLocation), Error> {
        let frames = symbols.frames(client)?;
        require(
            frames["truncated"] == false,
            "CAPABILITY_UNAVAILABLE",
            "source navigation needs complete bounded runtime frames",
        )?;
        let depth = frames["depth"]
            .as_u64()
            .ok_or_else(|| error("DEBUG_FRAMES", "runtime depth missing"))?;
        let registers = client.registers()?;
        Ok((depth, symbols.location(registers[11])?))
    }
    pub(in super::super) fn decide(
        &mut self,
        depth: u64,
        location: &SourceLocation,
        breakpoint: bool,
        stop: &Stop,
    ) -> Option<&'static str> {
        if !matches!(stop, Stop::Stopped { signal: 5 }) {
            return Some("signal");
        }
        if breakpoint {
            return Some("breakpoint");
        }
        if self.steps >= 10_000 || self.started.elapsed() >= Duration::from_secs(10) {
            return Some("limit");
        }
        if match self.kind {
            SourceNavigationKind::Next => {
                depth < self.depth
                    || (depth == self.depth
                        && location.file.is_some()
                        && location.line.is_some()
                        && (location.file != self.location.file
                            || location.line != self.location.line))
            }
            SourceNavigationKind::Finish => depth < self.depth,
        } {
            Some("complete")
        } else {
            None
        }
    }
    /// `None` means another actual step is needed; `Some` is a paused or terminal outcome.
    /// A limit never resumes a VM or fabricates a completed source operation.
    pub fn observe(
        &mut self,
        symbols: &ExactSymbols,
        client: &mut DebugClient,
        stop: &Stop,
    ) -> Result<Option<Value>, Error> {
        self.steps += 1;
        if !matches!(stop, Stop::Stopped { .. }) {
            return Ok(Some(
                json!({"navigation":self.kind,"status":"vm-terminal","steps":self.steps,"stop":stop}),
            ));
        }
        let (depth, location) = Self::snapshot(symbols, client)?;
        let pc = client.registers()?[11];
        let breakpoint = client.has_breakpoint(pc);
        Ok(self.decide(depth,&location,breakpoint,stop).map(|status|json!({"navigation":self.kind,"status":status,"steps":self.steps,"depth":depth,"source":location})))
    }
}
