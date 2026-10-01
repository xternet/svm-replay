use super::*;

impl ExactSymbols {
    pub fn variable(&self, client: &mut DebugClient, name: &str) -> Result<Value, Error> {
        require(
            !name.is_empty()
                && name.len() <= 128
                && name.chars().enumerate().all(|(i, c)| {
                    c == '_'
                        || if i == 0 {
                            c.is_alphabetic()
                        } else {
                            c.is_alphanumeric()
                        }
                }),
            "SYMBOL_CONFIG",
            "variable requires one identifier, not an expression",
        )?;
        self.bind_runtime(client)?;
        let before = client.registers()?;
        let unavailable =
            |reason: &str| json!({"name":name,"value":null,"available":false,"reason":reason});
        let functions: Vec<_> = self
            .functions
            .iter()
            .filter(|function| {
                function
                    .ranges
                    .iter()
                    .any(|range| before[11] >= range.begin && before[11] < range.end)
            })
            .collect();
        if functions.len() != 1 {
            return Ok(unavailable("current source function missing or ambiguous"));
        }
        let function = functions[0];
        if function.prologue_end.is_some_and(|end| before[11] < end) {
            return Ok(unavailable(
                "function prologue: source variable location not established",
            ));
        }
        let variables: Vec<_> = function
            .variables
            .iter()
            .filter(|variable| variable.name == name)
            .collect();
        if variables.is_empty() {
            return Ok(unavailable("missing variable debug information"));
        }
        if variables.len() != 1 || !variables[0].direct_scope {
            return Ok(unavailable(
                "unsupported or ambiguous nested lexical variable scope",
            ));
        }
        let unit = &self.units[function.unit];
        let dwarf = self
            .dwarf
            .as_ref()
            .ok_or_else(|| error("SYMBOL_FORMAT", "DWARF source missing"))?;
        let entry = unit.entry(variables[0].offset).map_err(dwarf_error)?;
        let Some(location) = entry
            .attr_value(gimli::DW_AT_location)
            .map_err(dwarf_error)?
        else {
            return Ok(unavailable("optimized-out or unsupported location/type"));
        };
        let expression = match location {
            AttributeValue::Exprloc(expression) => Some(expression),
            value => {
                let Some(mut locations) = dwarf.attr_locations(unit, value).map_err(dwarf_error)?
                else {
                    return Ok(unavailable("unsupported DWARF location form"));
                };
                let mut expression = None;
                let mut count = 0;
                while let Some(location) = locations.next().map_err(dwarf_error)? {
                    count += 1;
                    require(count <= 65535, "SYMBOL_LIMIT", "DWARF location-list bound")?;
                    if before[11] >= location.range.begin && before[11] < location.range.end {
                        require(
                            expression.is_none(),
                            "SYMBOL_FORMAT",
                            "overlapping variable locations",
                        )?;
                        expression = Some(location.data);
                    }
                }
                expression
            }
        };
        let Some(expression) = expression else {
            return Ok(unavailable(
                "optimized-out or unavailable variable location at this PC",
            ));
        };
        let Some(type_offset) = entry
            .attr_value(gimli::DW_AT_type)
            .map_err(dwarf_error)?
            .and_then(|value| match value {
                AttributeValue::UnitRef(offset) => Some(offset),
                _ => None,
            })
        else {
            return Ok(unavailable("unsupported variable type reference"));
        };
        let Some((size, signed, pointer, type_name)) = self.variable_type(unit, type_offset)?
        else {
            return Ok(unavailable("optimized-out or unsupported location/type"));
        };
        let mut operations = expression.operations(unit.encoding());
        let operation = operations.next().map_err(dwarf_error)?;
        if operations.next().map_err(dwarf_error)?.is_some() {
            return Ok(unavailable(
                "unsupported compound DWARF location expression",
            ));
        }
        let value = match operation {
            Some(gimli::Operation::Register { register }) if register.0 < 12 => {
                before[usize::from(register.0)]
            }
            Some(gimli::Operation::FrameOffset { offset }) => {
                let Some(base) = function.frame_base.as_ref() else {
                    return Ok(unavailable("DWARF frame base unavailable"));
                };
                let mut operations = base.clone().operations(unit.encoding());
                let base = operations.next().map_err(dwarf_error)?;
                if operations.next().map_err(dwarf_error)?.is_some() {
                    return Ok(unavailable("unsupported compound DWARF frame base"));
                }
                let Some(gimli::Operation::Register { register }) = base else {
                    return Ok(unavailable("unsupported DWARF frame-base operator"));
                };
                if register.0 >= 12 {
                    return Ok(unavailable("DWARF frame-base register unavailable"));
                }
                let address = before[usize::from(register.0)]
                    .checked_add_signed(offset)
                    .ok_or_else(|| error("SYMBOL_FORMAT", "DWARF frame address overflow"))?;
                let bytes = match client.memory(address, size) {
                    Ok(bytes) => bytes,
                    Err(error) if error.code == "DEBUG_TARGET" => {
                        return Ok(
                            json!({"name":name,"value":null,"available":false,"reason":"unreadable runtime variable location","targetError":error}),
                        )
                    }
                    Err(error) => return Err(error),
                };
                let mut raw = [0u8; 8];
                raw[..size].copy_from_slice(&bytes);
                u64::from_le_bytes(raw)
            }
            _ => return Ok(unavailable("unsupported DWARF location operator")),
        };
        require(
            client.state() == DebugState::Stopped && client.registers()? == before,
            "DEBUG_STALE_STATE",
            "VM changed during variable read",
        )?;
        let bits = (size * 8) as u32;
        let value = if pointer {
            format!("0x{value:x}")
        } else if signed {
            (((value << (64 - bits)) as i64) >> (64 - bits)).to_string()
        } else {
            (value & (u64::MAX >> (64 - bits))).to_string()
        };
        Ok(
            json!({"name":name,"value":value,"available":true,"type":type_name,"scope":"current paused VM frame; no arbitrary expression evaluation"}),
        )
    }
}
