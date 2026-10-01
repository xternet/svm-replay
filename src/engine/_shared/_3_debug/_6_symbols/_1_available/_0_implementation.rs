use super::*;

impl ExactSymbols {
    pub fn available(&self) -> bool {
        self.available
    }
}
impl ExactSymbols {
    pub fn elf_sha256(&self) -> &Digest {
        &self.elf_sha256
    }
}
impl ExactSymbols {
    pub fn admission(&self) -> Value {
        json!({"schema":"sbpf-exact-symbols/v1","elfSha256":self.elf_sha256,"path":self.path,"available":self.available,"reason":if self.available{Value::Null}else{json!("missing embedded source DWARF")}})
    }
}
impl ExactSymbols {
    pub(in super::super) fn ready(&self) -> Result<(), Error> {
        require(
            self.available,
            "CAPABILITY_UNAVAILABLE",
            "missing embedded source DWARF",
        )
    }
}
impl ExactSymbols {
    pub fn location(&self, pc: u64) -> Result<SourceLocation, Error> {
        self.ready()?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| error("SYMBOL_FORMAT", "admitted DWARF context missing"))?;
        let line = context.find_location(pc).map_err(dwarf_error)?;
        // Split/DWO debug data is intentionally never loaded from embedded paths.
        let mut frames = context
            .find_frames(pc)
            .skip_all_loads()
            .map_err(dwarf_error)?;
        let function = match frames.next().map_err(dwarf_error)? {
            Some(frame) => match frame.function {
                Some(name) => Some(name.raw_name().map_err(dwarf_error)?.into_owned()),
                None => None,
            },
            None => None,
        };
        require(
            function.as_ref().is_none_or(|name| name.len() <= 4096)
                && line
                    .as_ref()
                    .and_then(|line| line.file)
                    .is_none_or(|path| path.len() <= 4096),
            "SYMBOL_LIMIT",
            "source location display length exceeds bound",
        )?;
        Ok(SourceLocation {
            file: line.as_ref().and_then(|l| l.file.map(str::to_owned)),
            line: line.as_ref().and_then(|l| l.line),
            column: line.as_ref().and_then(|l| l.column),
            function,
        })
    }
}
impl ExactSymbols {
    pub fn line_addresses(&self, file: &str, line: u32) -> Result<Vec<u64>, Error> {
        self.ready()?;
        require(
            !file.is_empty() && file.len() <= 4096 && line > 0,
            "SYMBOL_CONFIG",
            "invalid source location",
        )?;
        let paths: BTreeSet<&str> = self
            .lines
            .iter()
            .filter(|entry| {
                entry.file == file
                    || Path::new(&entry.file)
                        .file_name()
                        .and_then(|name| name.to_str())
                        == Some(file)
            })
            .map(|entry| entry.file.as_str())
            .collect();
        require(
            paths.len() == 1,
            "CAPABILITY_UNAVAILABLE",
            "source file is absent or basename ambiguous; supply exact DWARF path",
        )?;
        let addresses: BTreeSet<u64> = self
            .lines
            .iter()
            .filter(|entry| paths.contains(entry.file.as_str()) && entry.line == line)
            .map(|entry| entry.address)
            .collect();
        require(
            !addresses.is_empty()
                && addresses.len() <= 256
                && addresses.iter().all(|pc| pc % 8 == 0),
            "CAPABILITY_UNAVAILABLE",
            "source line has no bounded aligned SBPF locations",
        )?;
        Ok(addresses.into_iter().collect())
    }
}
impl ExactSymbols {
    pub fn bind_runtime(&self, client: &mut DebugClient) -> Result<(), Error> {
        self.ready()?;
        require(
            client.state() == DebugState::Stopped,
            "DEBUG_STATE",
            "source operation requires stopped live VM",
        )?;
        let bytes = read_bounded_file(&self.path, 32 * 1024 * 1024).map_err(protocol_error)?;
        require(
            Digest::of(bytes) == self.elf_sha256,
            "SYMBOL_IDENTITY",
            "admitted ELF changed on disk",
        )?;
        let metadata = client.metadata()?;
        let (mode, sha) =
            if metadata.trim_start().starts_with('{') {
                let value = parse_json(metadata.as_bytes())?;
                require(
                    value["schema"] == "m14-source-control/v1",
                    "SYMBOL_IDENTITY",
                    "unknown controlled-source metadata schema",
                )?;
                (
                    value["executionMode"]
                        .as_str()
                        .ok_or_else(|| error("SYMBOL_IDENTITY", "runtime execution mode missing"))?
                        .to_owned(),
                    Digest::new(value["elfSha256"].as_str().ok_or_else(|| {
                        error("SYMBOL_IDENTITY", "runtime ELF identity missing")
                    })?)?,
                )
            } else {
                let value = InvocationMetadata::parse(&metadata)?;
                ("interpreter-debug".to_owned(), value.elf_sha256)
            };
        require(
            mode == "interpreter-debug" && sha == self.elf_sha256,
            "SYMBOL_IDENTITY",
            "runtime mode/ELF differs from admitted immutable symbols",
        )
    }
}
impl ExactSymbols {
    pub fn frames(&self, client: &mut DebugClient) -> Result<Value, Error> {
        self.bind_runtime(client)?;
        let before = client.registers()?;
        let raw = client.runtime_frames()?;
        require(
            raw.trim_start().starts_with('{'),
            "CAPABILITY_UNAVAILABLE",
            "runtime does not provide read-only frames endpoint",
        )?;
        let frames = RuntimeFrames::parse(&parse_json(raw.as_bytes())?)?;
        require(
            address(&frames.frames[0].pc)? == before[11]
                && address(&frames.frames[0].frame_pointer)? == before[10],
            "DEBUG_STALE_STATE",
            "runtime frames disagree with current registers",
        )?;
        for (expected, observed) in before[..11].iter().zip(&frames.frames[0].registers) {
            require(
                Some(*expected) == observed.as_deref().map(address).transpose()?,
                "DEBUG_STALE_STATE",
                "runtime frame register changed",
            )?;
        }
        let mut value = serde_json::to_value(&frames).map_err(dwarf_error)?;
        for (index, frame) in frames.frames.iter().enumerate() {
            let pc = address(&frame.pc)?;
            let lookup = if index == 0 {
                pc
            } else {
                pc.checked_sub(1)
                    .ok_or_else(|| error("DEBUG_FRAMES", "caller return PC underflow"))?
            };
            let source = self.location(lookup)?;
            value["frames"][index]["function"] = json!(source.function);
            value["frames"][index]["source"] =
                json!({"file":source.file,"line":source.line,"column":source.column});
        }
        require(
            client.state() == DebugState::Stopped && client.registers()? == before,
            "DEBUG_STALE_STATE",
            "VM changed during frame inspection",
        )?;
        Ok(value)
    }
}
