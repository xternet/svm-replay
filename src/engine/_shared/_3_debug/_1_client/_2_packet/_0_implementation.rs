use super::*;

impl DebugClient {
    pub(in super::super) fn packet(&mut self) -> Result<Option<Vec<u8>>, Error> {
        while self.input.first() == Some(&b'+') {
            self.input.remove(0);
        }
        if self.input.is_empty() {
            return Ok(None);
        }
        require(
            self.input[0] == b'$',
            "DEBUG_PROTOCOL",
            "unexpected RSP prefix or rejected command checksum",
        )?;
        let Some(end) = self.input.iter().position(|b| *b == b'#') else {
            require(
                self.input.len() <= self.maximum,
                "DEBUG_LIMIT",
                "unterminated packet exceeds bound",
            )?;
            return Ok(None);
        };
        if self.input.len() < end + 3 {
            return Ok(None);
        }
        let expected = hex(&[self.input[1..end]
            .iter()
            .fold(0_u8, |sum, b| sum.wrapping_add(*b))]);
        require(
            self.input[end + 1..end + 3].eq_ignore_ascii_case(expected.as_bytes()),
            "DEBUG_PROTOCOL",
            "RSP checksum differs",
        )?;
        let encoded = self.input[1..end].to_vec();
        self.input.drain(..end + 3);
        let mut decoded = Vec::new();
        let mut cursor = 0;
        while cursor < encoded.len() {
            let byte = encoded[cursor];
            cursor += 1;
            match byte {
                b'}' => {
                    require(
                        cursor < encoded.len(),
                        "DEBUG_PROTOCOL",
                        "unterminated RSP escape",
                    )?;
                    decoded.push(encoded[cursor] ^ 32);
                    cursor += 1;
                }
                b'*' => {
                    require(
                        cursor < encoded.len() && !decoded.is_empty(),
                        "DEBUG_PROTOCOL",
                        "invalid RSP repeat",
                    )?;
                    let count = i16::from(encoded[cursor]) - 29;
                    cursor += 1;
                    require(
                        (3..=97).contains(&count) && decoded.len() + count as usize <= self.maximum,
                        "DEBUG_LIMIT",
                        "invalid or oversized RSP repeat",
                    )?;
                    let previous = decoded[decoded.len() - 1];
                    decoded.resize(decoded.len() + count as usize, previous);
                }
                _ => decoded.push(byte),
            }
            require(
                decoded.len() <= self.maximum,
                "DEBUG_LIMIT",
                "decoded RSP packet exceeds bound",
            )?;
        }
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| error("DEBUG_PROTOCOL", "unsolicited RSP response"))?;
        pending.wire_bytes = pending
            .wire_bytes
            .checked_add(end + 3)
            .ok_or_else(|| error("DEBUG_LIMIT", "RSP byte count overflow"))?;
        if matches!(pending.kind, PendingKind::Monitor) {
            require(
                pending.wire_bytes <= self.maximum,
                "DEBUG_LIMIT",
                "cumulative monitor reply exceeds bound",
            )?;
        }
        self.write(b"+")?;
        Ok(Some(decoded))
    }
}
