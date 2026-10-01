use super::{
    covers, evidence_hashes, json_value, query_key, require, unique_hashes, validate_identity,
    validate_record, value_hash, HistoricalSource, Result,
};
use serde_json::Value;
use std::collections::BTreeMap;
use svm_replay_protocol::Digest;

/// Explicit trusted Bank inputs; semantic proof remains the preparer's responsibility.
pub struct SuppliedBankSource {
    identity: Value,
    records: BTreeMap<Digest, Value>,
}
impl SuppliedBankSource {
    pub fn new(identity: Value, inputs: Vec<Value>) -> Result<Self> {
        validate_identity(&identity)?;
        require(
            identity["kind"] == "supplied-bank",
            "INVALID_SOURCE",
            "supplied Bank source kind",
        )?;
        let mut records = BTreeMap::new();
        for mut record in inputs {
            json_value(&record)?;
            let key = query_key(&record["query"])?;
            require(
                record["query"]["kind"] == "bank-input" && covers(&identity, &record["query"])?,
                "INVALID_SOURCE",
                "supplied input capability/coverage",
            )?;
            validate_record(&record, &record["query"])?;
            require(
                !records.contains_key(&key),
                "INVALID_SOURCE",
                "duplicate supplied Bank input",
            )?;
            let digest = value_hash("m11-supplied-record/v1", &record)?;
            let mut evidence = evidence_hashes(&record)?;
            evidence.push(digest.as_str().to_owned());
            record["evidenceHashes"] = unique_hashes(evidence);
            records.insert(key, record);
        }
        Ok(Self { identity, records })
    }
}
impl HistoricalSource for SuppliedBankSource {
    fn identity(&self) -> Value {
        self.identity.clone()
    }
    fn diagnostics(&self) -> Result<Value> {
        Ok(super::local_diagnostics())
    }
    fn inspect(&self, query: &Value) -> Result<Option<Value>> {
        let key = query_key(query)?;
        if !covers(&self.identity, query)? {
            return Ok(None);
        }
        Ok(self.records.get(&key).cloned())
    }
}
