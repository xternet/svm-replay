use super::*;

impl Router {
    pub(in super::super) fn command(
        &mut self,
        command: DebugCommand,
        phase: &str,
        driver: &mut DebugDriver,
        events: &mut Vec<Value>,
    ) -> Result<(), Error> {
        require(
            command.phase == phase,
            "DEBUG_STALE_STATE",
            "command targets another replay phase",
        )?;
        require(
            driver.request_ids.len() < 32768 && driver.request_ids.insert(command.request_id),
            "DEBUG_CONFIG",
            "duplicate request ID or request limit exhausted",
        )?;
        if matches!(command.action, DebugAction::ContinueAll) {
            require(
                command.target.is_none(),
                "DEBUG_CONFIG",
                "continue-all must not name a single invocation",
            )?;
            self.all = true;
            let paused: Vec<_> = self
                .invocations
                .iter()
                .filter_map(|(key, vm)| (vm.client.state() == DebugState::Stopped).then_some(*key))
                .collect();
            for key in paused {
                self.start(key, "continue")?;
            }
            return driver.emit(json!({"kind":"response","phase":phase,"requestId":command.request_id,"result":{"continueAll":true}}),events);
        }
        let token = command.target.ok_or_else(|| {
            error(
                "DEBUG_CONFIG",
                "operation requires an explicit invocation and stop generation",
            )
        })?;
        let key = (token.execution_index, token.invocation_index);
        let invocation = self
            .invocations
            .get_mut(&key)
            .ok_or_else(|| error("DEBUG_ROUTING", "unknown live invocation"))?;
        require(
            invocation.generation == token.generation,
            "DEBUG_STALE_STATE",
            "command stop generation is stale",
        )?;
        require(
            invocation.navigation.is_none() || matches!(command.action, DebugAction::Pause),
            "DEBUG_STATE",
            "source navigation active; pause before another operation",
        )?;
        let result = match command.action {
            DebugAction::Registers => {
                json!({"registers":invocation.client.registers()?.map(|r|format!("0x{r:x}"))})
            }
            DebugAction::Memory { address, length } => {
                json!({"address":format!("0x{address:x}"),"dataHex":invocation.client.memory(address,length)?.iter().map(|b|format!("{b:02x}")).collect::<String>()})
            }
            DebugAction::Metadata => {
                let observed = InvocationMetadata::parse(&invocation.client.metadata()?)?;
                require(
                    observed.fields == invocation.metadata.fields,
                    "DEBUG_STALE_STATE",
                    "runtime identity changed while paused",
                )?;
                json!({"fields":observed.fields})
            }
            DebugAction::SourceLocation
            | DebugAction::Frames
            | DebugAction::SourceVariable { .. }
            | DebugAction::SourceBreakpoint { .. }
            | DebugAction::SourceNext
            | DebugAction::SourceFinish => {
                let symbols = driver
                    .symbols
                    .get(invocation.metadata.elf_sha256.as_str())
                    .ok_or_else(|| {
                        error(
                            "CAPABILITY_UNAVAILABLE",
                            "no caller-admitted exact ELF symbols for this invocation",
                        )
                    })?;
                match command.action {
                    DebugAction::SourceLocation => {
                        symbols.bind_runtime(&mut invocation.client)?;
                        let pc = invocation.client.registers()?[11];
                        json!(symbols.location(pc)?)
                    }
                    DebugAction::Frames => symbols.frames(&mut invocation.client)?,
                    DebugAction::SourceVariable { name } => {
                        symbols.variable(&mut invocation.client, &name)?
                    }
                    DebugAction::SourceBreakpoint {
                        file,
                        line,
                        enabled,
                    } => {
                        symbols.bind_runtime(&mut invocation.client)?;
                        let addresses = symbols.line_addresses(&file, line)?;
                        for pc in &addresses {
                            invocation.client.breakpoint(*pc, enabled)?;
                        }
                        json!({"file":file,"line":line,"enabled":enabled,"addresses":addresses})
                    }
                    DebugAction::SourceNext | DebugAction::SourceFinish => {
                        let kind = if matches!(command.action, DebugAction::SourceNext) {
                            SourceNavigationKind::Next
                        } else {
                            SourceNavigationKind::Finish
                        };
                        invocation.navigation = Some(SourceNavigation::begin(
                            symbols,
                            &mut invocation.client,
                            kind,
                        )?);
                        self.automatic.insert(key);
                        json!({"runningTarget":self.start(key,"step")?,"navigation":kind})
                    }
                    _ => return Err(error("DEBUG_CONFIG", "source routing invariant violated")),
                }
            }
            DebugAction::AccountDirectory => {
                bound_accounts(&mut invocation.client, &invocation.metadata, None)?
            }
            DebugAction::InspectAccount {
                pubkey,
                max_data_bytes,
            } => bound_accounts(
                &mut invocation.client,
                &invocation.metadata,
                Some((&pubkey, max_data_bytes)),
            )?,
            DebugAction::Breakpoint { pc, enabled } => {
                invocation.client.breakpoint(pc, enabled)?;
                json!({"pc":format!("0x{pc:x}"),"enabled":enabled})
            }
            DebugAction::Pause => {
                invocation.client.pause()?;
                json!({"interruptRequested":true,"state":"RUNNING"})
            }
            DebugAction::Step => json!({"runningTarget":self.start(key,"step")?}),
            DebugAction::Continue => json!({"runningTarget":self.start(key,"continue")?}),
            DebugAction::StepOver => {
                self.automatic.insert(key);
                json!({"runningTarget":self.start(key,"step")?,"navigation":"one actual SBPF instruction; descendant VMs continue automatically"})
            }
            DebugAction::StepOut => {
                let ancestors = invocation.metadata.identity.ancestors.clone();
                let parent = ancestors
                    .iter()
                    .rev()
                    .find_map(|id| {
                        self.invocations
                            .contains_key(&(key.0, *id))
                            .then_some((key.0, *id))
                    })
                    .ok_or_else(|| {
                        error(
                            "CAPABILITY_UNAVAILABLE",
                            "no attached SBPF caller; native-only boundary is not a paused VM",
                        )
                    })?;
                let caller = self
                    .invocations
                    .get_mut(&parent)
                    .ok_or_else(|| error("DEBUG_ROUTING", "attached caller missing"))?;
                require(
                    caller.client.state() == DebugState::Running && caller.command.is_some(),
                    "DEBUG_ROUTING",
                    "caller has no suspended execution command",
                )?;
                if caller.command == Some("continue") {
                    caller.client.pause()?;
                }
                self.automatic.insert(key);
                json!({"runningTarget":self.start(key,"continue")?,"returnTo":{"executionIndex":parent.0,"invocationIndex":parent.1},"navigation":"continue this invocation; observe actual caller stop/termination separately"})
            }
            DebugAction::Detach => {
                invocation.client.detach()?;
                return Err(error(
                    "CANCELLED",
                    "debugger detached before whole-worker completion",
                ));
            }
            DebugAction::ContinueAll => {
                return Err(error(
                    "DEBUG_CONFIG",
                    "continue-all routing invariant violated",
                ))
            }
        };
        driver.emit(json!({"kind":"response","phase":phase,"requestId":command.request_id,"target":token,"result":result}),events)
    }
}
