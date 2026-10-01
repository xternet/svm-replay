use super::*;

impl ExactSymbols {
    pub fn admit(path: &Path, expected: &Digest) -> Result<Self, Error> {
        let bytes = read_bounded_file(path, 32 * 1024 * 1024).map_err(protocol_error)?;
        require(
            Digest::of(&bytes) == *expected,
            "SYMBOL_IDENTITY",
            "exact ELF differs from immutable replay digest",
        )?;
        require(
            bytes.len() >= 64 && bytes.starts_with(b"\x7fELF\x02\x01\x01"),
            "SYMBOL_FORMAT",
            "expected little-endian ELF64",
        )?;
        let file = object::File::parse(bytes.as_slice()).map_err(dwarf_error)?;
        require(
            matches!(
                file.architecture(),
                object::Architecture::Bpf | object::Architecture::Sbf
            ),
            "CAPABILITY_UNAVAILABLE",
            "source adapter requires exact SBPF/BPF ELF",
        )?;
        require(
            file.sections().count() <= 4096,
            "SYMBOL_LIMIT",
            "ELF section count exceeds bound",
        )?;
        let mut total = 0usize;
        let dwarf = gimli::Dwarf::load(|id| -> Result<DwarfReader, Error> {
            let data = if let Some(section) = file.section_by_name(id.name()) {
                if let object::SectionFlags::Elf { sh_flags } = section.flags() {
                    require(
                        sh_flags & 0x800 == 0,
                        "CAPABILITY_UNAVAILABLE",
                        "compressed DWARF is outside this finite adapter",
                    )?;
                }
                section.data().map_err(dwarf_error)?
            } else {
                &[]
            };
            total = total
                .checked_add(data.len())
                .ok_or_else(|| error("SYMBOL_LIMIT", "DWARF size overflow"))?;
            require(
                total <= 64 * 1024 * 1024,
                "SYMBOL_LIMIT",
                "DWARF section byte budget exceeded",
            )?;
            Ok(EndianArcSlice::new(Arc::from(data), LittleEndian))
        })?;
        let available =
            !dwarf.debug_info.reader().is_empty() && !dwarf.debug_line.reader().is_empty();
        let mut symbols = Self {
            path: path.canonicalize().map_err(dwarf_error)?,
            elf_sha256: expected.clone(),
            available,
            dwarf: None,
            context: None,
            units: Vec::new(),
            functions: Vec::new(),
            lines: Vec::new(),
        };
        if !available {
            return Ok(symbols);
        }
        let dwarf = Arc::new(dwarf);
        let mut units = dwarf.units();
        let mut entries_seen = 0usize;
        let mut display_bytes = 0usize;
        while let Some(header) = units.next().map_err(dwarf_error)? {
            require(
                symbols.units.len() < 4096,
                "SYMBOL_LIMIT",
                "DWARF compilation unit limit",
            )?;
            let unit = dwarf.unit(header).map_err(dwarf_error)?;
            let unit_index = symbols.units.len();
            let mut rows_prologue = Vec::new();
            if let Some(program) = unit.line_program.clone() {
                let mut rows = program.rows();
                while let Some((header, row)) = rows.next_row().map_err(dwarf_error)? {
                    require(
                        symbols.lines.len() < 262_144,
                        "SYMBOL_LIMIT",
                        "DWARF line row limit",
                    )?;
                    if row.end_sequence() {
                        continue;
                    }
                    if row.prologue_end() {
                        rows_prologue.push(row.address());
                    }
                    if row.is_stmt() {
                        if let (Some(file), Some(line)) = (row.file(header), row.line()) {
                            let name = text(&dwarf, &unit, file.path_name())?;
                            let mut path = PathBuf::from(&name);
                            if !path.is_absolute() {
                                if let Some(directory) = file.directory(header) {
                                    let directory = text(&dwarf, &unit, directory)?;
                                    path = PathBuf::from(directory).join(path);
                                }
                                if !path.is_absolute() {
                                    if let Some(directory) = unit.comp_dir.as_ref() {
                                        let directory =
                                            directory.to_string_lossy().map_err(dwarf_error)?;
                                        require(
                                            directory.len() <= 4096,
                                            "SYMBOL_LIMIT",
                                            "DWARF compilation path exceeds display bound",
                                        )?;
                                        path = PathBuf::from(directory.as_ref()).join(path);
                                    }
                                }
                            }
                            // Path joining is lexical only; no stat, read, canonicalize or script import.
                            let file = path.to_string_lossy().into_owned();
                            require(
                                file.len() <= 4096,
                                "SYMBOL_LIMIT",
                                "joined DWARF path exceeds display bound",
                            )?;
                            display_bytes += file.len();
                            require(
                                display_bytes <= 16 * 1024 * 1024,
                                "SYMBOL_LIMIT",
                                "DWARF materialized display-string budget exceeded",
                            )?;
                            symbols.lines.push(Line {
                                address: row.address(),
                                file,
                                line: u32::try_from(line.get()).map_err(dwarf_error)?,
                            });
                        }
                    }
                }
            }
            let mut entries = unit.entries();
            let mut depth = 0isize;
            let mut stack: Vec<(isize, usize)> = Vec::new();
            while let Some((delta, entry)) = entries.next_dfs().map_err(dwarf_error)? {
                entries_seen += 1;
                require(
                    entries_seen <= 262_144,
                    "SYMBOL_LIMIT",
                    "DWARF entry budget exhausted",
                )?;
                depth += delta;
                require(
                    (0..=256).contains(&depth),
                    "SYMBOL_LIMIT",
                    "DWARF tree depth exceeds bound",
                )?;
                while stack.last().is_some_and(|(prior, _)| *prior >= depth) {
                    stack.pop();
                }
                if entry.tag() == gimli::DW_TAG_subprogram {
                    let mut ranges = Vec::new();
                    let mut iterator = dwarf.die_ranges(&unit, entry).map_err(dwarf_error)?;
                    while let Some(range) = iterator.next().map_err(dwarf_error)? {
                        require(
                            ranges.len() < 1024 && range.begin < range.end,
                            "SYMBOL_LIMIT",
                            "invalid or excessive function ranges",
                        )?;
                        ranges.push(range);
                    }
                    if ranges.is_empty() {
                        continue;
                    }
                    let prologue_end = rows_prologue
                        .iter()
                        .copied()
                        .filter(|pc| {
                            ranges
                                .iter()
                                .any(|range| *pc >= range.begin && *pc < range.end)
                        })
                        .min();
                    let frame_base = match entry
                        .attr_value(gimli::DW_AT_frame_base)
                        .map_err(dwarf_error)?
                    {
                        Some(AttributeValue::Exprloc(expression)) => Some(expression),
                        _ => None,
                    };
                    let index = symbols.functions.len();
                    symbols.functions.push(Function {
                        unit: unit_index,
                        ranges,
                        frame_base,
                        variables: Vec::new(),
                        prologue_end,
                    });
                    stack.push((depth, index));
                } else if matches!(
                    entry.tag(),
                    gimli::DW_TAG_variable | gimli::DW_TAG_formal_parameter
                ) {
                    if let Some((function_depth, index)) = stack.last().copied() {
                        if let Some(name) =
                            entry.attr_value(gimli::DW_AT_name).map_err(dwarf_error)?
                        {
                            let name = text(&dwarf, &unit, name)?;
                            display_bytes += name.len();
                            require(
                                display_bytes <= 16 * 1024 * 1024,
                                "SYMBOL_LIMIT",
                                "DWARF materialized display-string budget exceeded",
                            )?;
                            symbols.functions[index].variables.push(Variable {
                                name,
                                offset: entry.offset(),
                                direct_scope: depth == function_depth + 1,
                            });
                        }
                    }
                }
            }
            symbols.units.push(unit);
        }
        symbols.context =
            Some(addr2line::Context::from_arc_dwarf(dwarf.clone()).map_err(dwarf_error)?);
        symbols.dwarf = Some(dwarf);
        Ok(symbols)
    }
}
