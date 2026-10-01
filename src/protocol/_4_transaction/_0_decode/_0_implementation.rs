use super::*;

pub(in super::super) fn invalid(message: &str) -> Error {
    Error::new("TRANSACTION_IDENTITY", message)
}

pub(in super::super) struct Reader<'a> {
    pub(in super::super) bytes: &'a [u8],
    pub(in super::super) position: usize,
}

impl<'a> Reader<'a> {
    pub(in super::super) fn take(&mut self, size: usize) -> Result<&'a [u8], Error> {
        let end = self
            .position
            .checked_add(size)
            .ok_or_else(|| invalid("length overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| invalid("truncated transaction"))?;
        self.position = end;
        Ok(bytes)
    }
    pub(in super::super) fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    pub(in super::super) fn shortvec(&mut self) -> Result<usize, Error> {
        let mut value = 0;
        for i in 0..3 {
            let part = self.byte()?;
            if i == 2 && part > 3 {
                return Err(invalid("shortvec overflow"));
            }
            value |= usize::from(part & 127) << (7 * i);
            if part & 128 == 0 {
                if i > 0 && part == 0 {
                    return Err(invalid("noncanonical shortvec"));
                }
                return Ok(value);
            }
        }
        Err(invalid("shortvec overflow"))
    }
    pub(in super::super) fn indexes(&mut self) -> Result<Vec<u8>, Error> {
        let size = self.shortvec()?;
        Ok(self.take(size)?.to_vec())
    }
    pub(in super::super) fn address(&mut self, size: usize) -> Result<String, Error> {
        Ok(bs58::encode(self.take(size)?).into_string())
    }
}

pub fn decode(encoded: &str) -> Result<Value, Error> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| invalid(&e.to_string()))?;
    if STANDARD.encode(&bytes) != encoded {
        return Err(invalid("noncanonical base64"));
    }
    if bytes.first() == Some(&129) {
        return super::super::_1_v1::decode(&bytes);
    }
    let mut r = Reader {
        bytes: &bytes,
        position: 0,
    };
    let signature_count = r.shortvec()?;
    let mut signatures = Vec::new();
    for _ in 0..signature_count {
        signatures.push(r.address(64)?);
    }
    let prefix = r.byte()?;
    let versioned = prefix & 128 != 0;
    if versioned && prefix != 128 {
        return Err(Error::new(
            "UNSUPPORTED_TRANSACTION_VERSION",
            "only legacy, v0 and v1 wire formats are reviewed",
        ));
    }
    let required = if versioned { r.byte()? } else { prefix };
    let readonly_signed = r.byte()?;
    let readonly_unsigned = r.byte()?;
    let key_count = r.shortvec()?;
    if usize::from(required) != signature_count
        || signature_count == 0
        || signature_count > key_count
        || usize::from(readonly_signed) >= signature_count
        || usize::from(readonly_unsigned) > key_count - signature_count
    {
        return Err(invalid("malformed message header"));
    }
    let mut keys = Vec::new();
    for _ in 0..key_count {
        keys.push(r.address(32)?);
    }
    let blockhash = r.address(32)?;
    let count = r.shortvec()?;
    let mut instructions = Vec::new();
    for _ in 0..count {
        let program = r.byte()?;
        let accounts = r.indexes()?;
        let data = r.indexes()?;
        instructions.push(json!({"programIdIndex":program,"accounts":accounts,"data":bs58::encode(data).into_string()}));
    }
    let mut lookups = Vec::new();
    let mut account_count = key_count;
    if versioned {
        let count = r.shortvec()?;
        for _ in 0..count {
            let key = r.address(32)?;
            let writable = r.indexes()?;
            let readonly = r.indexes()?;
            account_count += writable.len() + readonly.len();
            lookups.push(
                json!({"accountKey":key,"writableIndexes":writable,"readonlyIndexes":readonly}),
            );
        }
    }
    if r.position != bytes.len() || account_count > 256 {
        return Err(invalid("trailing bytes or too many accounts"));
    }
    for instruction in &instructions {
        let program = instruction["programIdIndex"]
            .as_u64()
            .ok_or_else(|| invalid("invalid program index"))?;
        let accounts = instruction["accounts"]
            .as_array()
            .ok_or_else(|| invalid("invalid instruction accounts"))?;
        if program == 0
            || program >= key_count as u64
            || accounts
                .iter()
                .any(|v| v.as_u64().is_none_or(|index| index >= account_count as u64))
        {
            return Err(invalid("instruction account index out of bounds"));
        }
    }
    Ok(
        json!({"version":if versioned {json!(0)}else{json!("legacy")},"transaction":{"signatures":signatures,"message":{
        "header":{"numRequiredSignatures":required,"numReadonlySignedAccounts":readonly_signed,"numReadonlyUnsignedAccounts":readonly_unsigned},
        "accountKeys":keys,"recentBlockhash":blockhash,"instructions":instructions,"addressTableLookups":lookups}}}),
    )
}

pub fn assert_historical(encoded: &str, archive: &Value) -> Result<(), Error> {
    let decoded = decode(encoded)?;
    let transaction = archive
        .get("transaction")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("missing archived transaction"))?;
    let message = transaction
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("missing archived message"))?;
    let keys = message
        .get("accountKeys")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing archived keys"))?;
    let keys = keys
        .iter()
        .map(|key| match key {
            Value::String(key) => Ok(key.clone()),
            Value::Object(key) => key
                .get("pubkey")
                .and_then(Value::as_str)
                .map(String::from)
                .ok_or_else(|| invalid("malformed archived key")),
            _ => Err(invalid("malformed archived key")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let instructions = message
        .get("instructions")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing archived instructions"))?;
    let instructions=instructions.iter().map(|instruction| {
        let obj=instruction.as_object().ok_or_else(|| invalid("malformed instruction"))?;
        for field in ["programIdIndex","accounts","data"] { if !obj.contains_key(field) {return Err(invalid("missing instruction field"));} }
        Ok(json!({"programIdIndex":obj["programIdIndex"],"accounts":obj["accounts"],"data":obj["data"]}))
    }).collect::<Result<Vec<_>,_>>()?;
    // Legacy RPC omits lookups; this is a specified empty wire section, not state fallback.
    let lookups = match message.get("addressTableLookups") {
        Some(v) => v.clone(),
        None => json!([]),
    };
    let mut canonical = json!({"signatures":transaction.get("signatures"),"message":{
        "header":message.get("header"),"accountKeys":keys,"recentBlockhash":message.get("recentBlockhash"),
        "instructions":instructions,"addressTableLookups":lookups}});
    if decoded["version"] == json!(1) {
        canonical["message"]["transactionConfig"] = message
            .get("transactionConfig")
            .ok_or_else(|| invalid("missing archived v1 transaction configuration"))?
            .clone();
    }
    if decoded["transaction"] != canonical
        || archive
            .get("version")
            .is_some_and(|version| *version != decoded["version"])
    {
        return Err(invalid(
            "wire transaction differs from archived message/signatures/version",
        ));
    }
    Ok(())
}
