use super::*;

pub(in super::super) struct Invocation {
    pub(in super::super) metadata: InvocationMetadata,
    pub(in super::super) client: DebugClient,
    pub(in super::super) generation: u64,
    pub(in super::super) command: Option<&'static str>,
    pub(in super::super) navigation: Option<SourceNavigation>,
}

pub(in super::super) struct Router {
    pub(in super::super) invocations: BTreeMap<(u16, u16), Invocation>,
    pub(in super::super) automatic: BTreeSet<(u16, u16)>,
    pub(in super::super) all: bool,
    pub(in super::super) executions: BTreeSet<u16>,
    pub(in super::super) max_invocations: usize,
}
impl Router {
    pub(in super::super) fn accept(
        &mut self,
        client: DebugClient,
        metadata: InvocationMetadata,
        phase: &str,
        driver: &mut DebugDriver,
        events: &mut Vec<Value>,
    ) -> Result<(), Error> {
        let execution = metadata.execution_index;
        let identity = &metadata.identity;
        let key = (execution, identity.invocation_index);
        require(
            !self.invocations.contains_key(&key),
            "DEBUG_ROUTING",
            "runtime reused invocation index",
        )?;
        if !self.executions.contains(&execution) {
            require(
                self.executions.len() < 64
                    && self
                        .executions
                        .last()
                        .is_none_or(|prior| *prior < execution),
                "DEBUG_ROUTING",
                "execution ordinal reused/out of order or limit exceeded",
            )?;
            self.executions.insert(execution);
        }
        require(
            self.invocations
                .keys()
                .filter(|(index, _)| *index == execution)
                .count()
                < self.max_invocations,
            "DEBUG_LIMIT",
            "execution invocation limit exceeded",
        )?;
        for (position, ancestor) in identity.ancestors.iter().enumerate() {
            if let Some(parent) = self.invocations.get(&(execution, *ancestor)) {
                require(
                    parent.client.state() == DebugState::Running
                        && parent.metadata.identity.ancestors == identity.ancestors[..position],
                    "DEBUG_ROUTING",
                    "attached ancestor is not suspended with matching ancestry",
                )?;
            }
        }
        if let Some(parent) = identity
            .parent_index
            .and_then(|id| self.invocations.get(&(execution, id)))
        {
            require(
                Some(&parent.metadata.identity.program) == identity.caller_program.as_ref()
                    && parent.metadata.identity.depth + 1 == identity.depth,
                "DEBUG_ROUTING",
                "runtime caller identity differs",
            )?;
        }
        let automatic = self.all
            || identity
                .ancestors
                .iter()
                .any(|id| self.automatic.contains(&(execution, *id)));
        let mut event = json!({"kind":"entry","phase":phase,"target":PauseToken{execution_index:execution,invocation_index:identity.invocation_index,generation:0},"identity":identity,"elfSha256":metadata.elf_sha256,"state":if automatic{"RUNNING"}else{"STOPPED"},"symbols":"assembly-only"});
        if let Some(symbols) = driver.symbols.get(metadata.elf_sha256.as_str()) {
            event["symbols"] = symbols.admission();
        }
        self.invocations.insert(
            key,
            Invocation {
                metadata,
                client,
                generation: 0,
                command: None,
                navigation: None,
            },
        );
        if automatic {
            event["target"] = json!(self.start(key, "continue")?);
        }
        driver.emit(event, events)
    }
}
impl Router {
    pub(in super::super) fn start(
        &mut self,
        key: (u16, u16),
        command: &'static str,
    ) -> Result<PauseToken, Error> {
        let invocation = self
            .invocations
            .get_mut(&key)
            .ok_or_else(|| error("DEBUG_ROUTING", "unknown invocation"))?;
        if command == "step" {
            invocation.client.step()?;
        } else {
            invocation.client.resume()?;
        }
        invocation.command = Some(command);
        invocation.generation = invocation
            .generation
            .checked_add(1)
            .ok_or_else(|| error("DEBUG_LIMIT", "stop generation overflow"))?;
        Ok(PauseToken {
            execution_index: key.0,
            invocation_index: key.1,
            generation: invocation.generation,
        })
    }
}
impl Router {
    pub(in super::super) fn poll(
        &mut self,
        phase: &str,
        driver: &mut DebugDriver,
        events: &mut Vec<Value>,
    ) -> Result<(), Error> {
        for (key, invocation) in &mut self.invocations {
            if invocation.client.state() != DebugState::Running {
                continue;
            }
            if let Some(stop) = invocation.client.poll_stop()? {
                let mut navigation = Value::Null;
                if let Some(mut active) = invocation.navigation.take() {
                    let symbols = driver
                        .symbols
                        .get(invocation.metadata.elf_sha256.as_str())
                        .ok_or_else(|| {
                            error("SYMBOL_IDENTITY", "active source symbols disappeared")
                        })?;
                    if let Some(outcome) = active.observe(symbols, &mut invocation.client, &stop)? {
                        navigation = outcome;
                    } else {
                        invocation.client.step()?;
                        invocation.generation = invocation
                            .generation
                            .checked_add(1)
                            .ok_or_else(|| error("DEBUG_LIMIT", "stop generation overflow"))?;
                        invocation.navigation = Some(active);
                        continue;
                    }
                }
                invocation.command = None;
                self.automatic.remove(key);
                driver.emit(json!({"kind":"stop","phase":phase,"target":PauseToken{execution_index:key.0,invocation_index:key.1,generation:invocation.generation},"stop":stop,"state":invocation.client.state(),"sourceNavigation":navigation}),events)?;
            }
        }
        Ok(())
    }
}
