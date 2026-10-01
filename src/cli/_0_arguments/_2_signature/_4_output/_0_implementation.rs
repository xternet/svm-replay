use super::*;

pub(in super::super) fn validate_fields(fields: &[String]) -> Result<(), Error> {
    for field in fields {
        if ![
            "status",
            "logs",
            "computeUnits",
            "accountTransitions",
            "trace",
        ]
        .contains(&field.as_str())
        {
            return Err(Error::new(
                "INVALID_REQUEST",
                format!("unknown result field: {field}"),
            ));
        }
    }
    Ok(())
}
pub(in super::super) fn finish(
    mut receipt: Value,
    signature: &str,
    fields: &[String],
    out: Option<&Path>,
) -> Result<Value, Error> {
    if receipt["outcome"] == "COMPLETED" {
        let path = receipt["receiptPath"]
            .as_str()
            .and_then(|p| Path::new(p).parent())
            .ok_or_else(|| Error::new("OUTPUT_INTEGRITY", "receipt location missing"))?
            .to_path_buf();
        if receipt["output"]["file"] != "result.json" {
            return Err(Error::new("OUTPUT_INTEGRITY", "unexpected result filename"));
        }
        let bytes = read_bounded_file(&path.join("result.json"), 256 * 1024 * 1024)
            .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
        if receipt["output"]["sha256"] != json!(Digest::of(&bytes)) {
            return Err(Error::new("OUTPUT_INTEGRITY", "result hash differs"));
        }
        let mut result = parse_json(&bytes)?;
        if !fields.is_empty() {
            for name in [
                "original",
                "replacement",
                "accountOverride",
                "requestedOverrides",
            ] {
                if let Some(object) = result.get_mut(name).and_then(Value::as_object_mut) {
                    object.retain(|key, _| fields.contains(key));
                }
            }
        }
        receipt["result"] = result;
        if fields.is_empty() || fields.iter().any(|field| field == "trace") {
            if let Some(exports) = receipt
                .pointer_mut("/trace/exports")
                .and_then(Value::as_array_mut)
            {
                for export in exports {
                    let name = export["file"]
                        .as_str()
                        .filter(|name| {
                            ["trace-original-control.json", "trace-requested.json"].contains(name)
                        })
                        .ok_or_else(|| {
                            Error::new("OUTPUT_INTEGRITY", "unexpected trace filename")
                        })?;
                    let bytes = read_bounded_file(&path.join(name), 256 * 1024 * 1024)
                        .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?;
                    if export["sha256"] != json!(Digest::of(&bytes)) {
                        return Err(Error::new("OUTPUT_INTEGRITY", "trace hash differs"));
                    }
                    export["data"] = parse_json(&bytes)?;
                }
            }
        }
    }
    if let Some(directory) = out {
        svm_replay_engine::_5_finalize::ensure_private_directory(directory)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::new("OUTPUT_IO", e.to_string()))?
            .as_millis();
        let name = if svm_replay_engine::shared::history::address(signature, 64).is_ok() {
            signature
        } else {
            "invalid-transaction"
        };
        let path = directory.join(format!("{stamp}-{name}.json"));
        crate::request::save_bytes(&path, &crate::json_output::encode_file(&receipt)?)?;
        let mut summary = json!({"outcome":receipt["outcome"],"savedTo":path,"receiptPath":receipt["receiptPath"],"error":receipt["error"]});
        if let Some(incomplete) = receipt.get("incomplete") {
            summary["incomplete"] = incomplete.clone();
        }
        if receipt["schema"] == "svm-replay-cli-error/v1" {
            summary["schema"] = receipt["schema"].clone();
        }
        Ok(summary)
    } else {
        Ok(receipt)
    }
}
