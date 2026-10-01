use super::*;

impl ExactSymbols {
    pub(in super::super) fn variable_type(
        &self,
        unit: &gimli::Unit<DwarfReader>,
        mut offset: gimli::UnitOffset<usize>,
    ) -> Result<Option<(usize, bool, bool, String)>, Error> {
        let dwarf = self
            .dwarf
            .as_ref()
            .ok_or_else(|| error("SYMBOL_FORMAT", "DWARF source missing"))?;
        let mut name = None;
        for _ in 0..64 {
            let entry = unit.entry(offset).map_err(dwarf_error)?;
            if name.is_none() {
                if let Some(value) = entry.attr_value(gimli::DW_AT_name).map_err(dwarf_error)? {
                    name = Some(text(dwarf, unit, value)?);
                }
            }
            if matches!(
                entry.tag(),
                gimli::DW_TAG_typedef
                    | gimli::DW_TAG_const_type
                    | gimli::DW_TAG_volatile_type
                    | gimli::DW_TAG_restrict_type
            ) {
                match entry.attr_value(gimli::DW_AT_type).map_err(dwarf_error)? {
                    Some(AttributeValue::UnitRef(next)) => {
                        offset = next;
                        continue;
                    }
                    _ => return Ok(None),
                }
            }
            let pointer = entry.tag() == gimli::DW_TAG_pointer_type;
            if !pointer && entry.tag() != gimli::DW_TAG_base_type {
                return Ok(None);
            }
            let size = match entry
                .attr_value(gimli::DW_AT_byte_size)
                .map_err(dwarf_error)?
                .and_then(|value| value.udata_value())
            {
                Some(size) => size,
                None if pointer => u64::from(unit.encoding().address_size),
                None => return Ok(None),
            };
            if ![1, 2, 4, 8].contains(&size) {
                return Ok(None);
            }
            let encoding = entry
                .attr_value(gimli::DW_AT_encoding)
                .map_err(dwarf_error)?;
            let signed = matches!(
                encoding,
                Some(AttributeValue::Encoding(
                    gimli::DW_ATE_signed | gimli::DW_ATE_signed_char
                ))
            );
            if !pointer
                && !signed
                && !matches!(
                    encoding,
                    Some(AttributeValue::Encoding(
                        gimli::DW_ATE_unsigned
                            | gimli::DW_ATE_unsigned_char
                            | gimli::DW_ATE_boolean
                    ))
                )
            {
                return Ok(None);
            }
            let name = match name {
                Some(name) => name,
                None if pointer => "pointer".into(),
                None => return Ok(None),
            };
            return Ok(Some((size as usize, signed, pointer, name)));
        }
        Err(error("SYMBOL_LIMIT", "DWARF type chain exceeds bound"))
    }
}
