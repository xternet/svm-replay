use super::*;

pub(in super::super) fn unique_hashes(values: impl IntoIterator<Item = String>) -> Value {
    Value::Array(
        values
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(Value::String)
            .collect(),
    )
}

pub(in super::super) fn evidence_hashes(record: &Value) -> Result<Vec<String>> {
    let hashes = record["evidenceHashes"]
        .as_array()
        .ok_or_else(|| SourceError::new("SOURCE_INTEGRITY", "missing evidence hashes"))?;
    require(
        !hashes.is_empty(),
        "SOURCE_INTEGRITY",
        "missing evidence hashes",
    )?;
    hashes
        .iter()
        .map(|v| hash(v, "SOURCE_INTEGRITY", "evidence hash").map(|d| d.as_str().to_owned()))
        .collect()
}

pub(in super::super) fn canonical_base64(value: &Value, label: &str) -> Result<Vec<u8>> {
    let encoded = text(value, "SOURCE_INTEGRITY", label)?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| SourceError::new("SOURCE_INTEGRITY", label))?;
    require(
        STANDARD.encode(&bytes) == encoded,
        "SOURCE_INTEGRITY",
        label,
    )?;
    Ok(bytes)
}

pub(in super::super) fn parse_source_json(bytes: &[u8]) -> Result<Value> {
    parse_json(bytes).map_err(|e| {
        SourceError::new(
            "SOURCE_INTEGRITY",
            format!("captured source JSON malformed: {e}"),
        )
    })
}

pub(in super::super) fn validate_record(record: &Value, query: &Value) -> Result<()> {
    json_value(record)?;
    let mut fields = vec!["query", "value", "evidenceHashes"];
    if record.get("provenance").is_some() {
        fields.push("provenance");
    }
    object(record, &fields, "SOURCE_INTEGRITY", "source record")?;
    require(
        query_key(&record["query"])? == query_key(query)?,
        "SOURCE_CONTEXT_MISMATCH",
        "returned query mismatch",
    )?;
    evidence_hashes(record)?;
    if record.get("provenance").is_some() {
        alchemy::validate_provenance(record, query)?;
    }
    let value = &record["value"];
    if query["kind"] == "account" {
        let mut fields = vec!["pubkey", "sourceSlot", "role", "presence"];
        if value["presence"] == "present" {
            fields.extend(["lamports", "owner", "executable", "rentEpoch", "dataBase64"]);
        }
        object(value, &fields, "SOURCE_INTEGRITY", "account image")?;
        let boundary_matches = if query["phase"] == "last-write-at-or-before-slot" {
            slot(
                &value["sourceSlot"],
                "SOURCE_INTEGRITY",
                "account write slot",
            )? <= slot(&query["slot"], "INVALID_QUERY", "slot")?
        } else {
            value["sourceSlot"] == query["slot"]
        };
        require(
            value["pubkey"] == query["pubkey"] && boundary_matches,
            "SOURCE_CONTEXT_MISMATCH",
            "account boundary mismatch",
        )?;
        require(
            [
                "application",
                "program",
                "programdata",
                "address-lookup-table",
                "sysvar",
            ]
            .iter()
            .any(|role| value["role"] == *role)
                && (value["presence"] == "present" || value["presence"] == "absent"),
            "SOURCE_INTEGRITY",
            "account role/presence",
        )?;
        if value["presence"] == "present" {
            address(&value["owner"], 32, "SOURCE_INTEGRITY", "account owner")?;
            require(
                value["executable"].is_boolean(),
                "SOURCE_INTEGRITY",
                "account executable/data",
            )?;
            canonical_base64(&value["dataBase64"], "account executable/data")?;
            for field in ["lamports", "rentEpoch"] {
                let decimal = text(
                    &value[field],
                    "SOURCE_INTEGRITY",
                    &format!("account {field}"),
                )?;
                require(
                    decimal == "0"
                        || (!decimal.starts_with('0')
                            && !decimal.is_empty()
                            && decimal.len() <= 20
                            && decimal.bytes().all(|b| b.is_ascii_digit())
                            && decimal.parse::<u64>().is_ok()),
                    "SOURCE_INTEGRITY",
                    &format!("account {field}"),
                )?;
            }
        }
    } else if query["kind"] == "block" {
        object(value, &["rawBase64"], "SOURCE_INTEGRITY", "raw block")?;
        let bytes = canonical_base64(&value["rawBase64"], "raw block bytes")?;
        require(
            Digest::of(&bytes)
                == hash(
                    &query["blockEvidenceSha256"],
                    "INVALID_QUERY",
                    "block evidence hash",
                )?,
            "SOURCE_INTEGRITY",
            "raw block digest",
        )?;
        let envelope = parse_source_json(&bytes)?;
        require(
            envelope.is_object()
                && envelope["result"].is_object()
                && envelope.get("error").is_none()
                && envelope["result"]["transactions"].is_array(),
            "SOURCE_INTEGRITY",
            "complete block envelope",
        )?;
        require(
            slot(
                &envelope["result"]["parentSlot"],
                "SOURCE_INTEGRITY",
                "block parent",
            )? < slot(&query["slot"], "INVALID_QUERY", "slot")?,
            "SOURCE_CONTEXT_MISMATCH",
            "block parent order",
        )?;
        address(
            &envelope["result"]["blockhash"],
            32,
            "SOURCE_INTEGRITY",
            "block hash",
        )?;
    } else if query["kind"] == "transaction" {
        let encoded = text(value, "SOURCE_INTEGRITY", "transaction bytes")?;
        let decoded = svm_replay_protocol::transaction::decode(encoded).map_err(|e| {
            SourceError::new(
                "SOURCE_INTEGRITY",
                format!("invalid transaction wire bytes: {e}"),
            )
        })?;
        require(
            decoded["transaction"]["signatures"][0] == query["signature"],
            "SOURCE_CONTEXT_MISMATCH",
            "transaction signature mismatch",
        )?;
    } else {
        require(
            value.is_object(),
            "SOURCE_INTEGRITY",
            "Bank evidence object required; semantic proof remains M9 validation",
        )?;
    }
    Ok(())
}
