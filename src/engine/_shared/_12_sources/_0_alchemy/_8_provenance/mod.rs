use super::*;

pub(in super::super) fn validate_provenance(record: &Value, query: &Value) -> Result<()> {
    let p = &record["provenance"];
    object(
        p,
        &[
            "schema",
            "request",
            "requestSha256",
            "responseSha256",
            "responseBytes",
            "responseBodyBase64",
            "genesisResponseSha256",
            "genesisResponseBodyBase64",
        ],
        "SOURCE_INTEGRITY",
        "RPC provenance",
    )?;
    require(
        p["schema"] == "m17-alchemy-rpc-provenance/v1"
            && response::request_matches(query, &p["request"])?,
        "SOURCE_INTEGRITY",
        "RPC provenance request mismatch",
    )?;
    let raw = canonical_base64(&p["responseBodyBase64"], "RPC provenance response")?;
    let genesis = canonical_base64(&p["genesisResponseBodyBase64"], "RPC provenance genesis")?;
    let request_hash = Digest::of(
        serde_json::to_vec(&p["request"])
            .map_err(|_| SourceError::new("SOURCE_INTEGRITY", "RPC request serialization"))?,
    );
    require(
        p["requestSha256"] == json!(request_hash)
            && p["responseSha256"] == json!(Digest::of(&raw))
            && p["responseBytes"].as_u64() == Some(raw.len() as u64)
            && p["genesisResponseSha256"] == json!(Digest::of(&genesis)),
        "SOURCE_INTEGRITY",
        "RPC provenance digest/length mismatch",
    )?;
    checked_genesis(&genesis, &query["genesisHash"])?;
    _11_lookup::validate_shape(query, &p["request"], &raw)?;
    require(
        value_from_response(query, &raw)? == record["value"],
        "SOURCE_INTEGRITY",
        "RPC provenance normalized value mismatch",
    )?;
    let evidence = evidence_hashes(record)?;
    for field in ["requestSha256", "responseSha256", "genesisResponseSha256"] {
        require(
            evidence
                .iter()
                .any(|h| Some(h.as_str()) == p[field].as_str()),
            "SOURCE_INTEGRITY",
            "RPC provenance evidence hash missing",
        )?;
    }
    Ok(())
}
