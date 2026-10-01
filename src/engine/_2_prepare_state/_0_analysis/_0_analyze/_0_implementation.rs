use super::*;

pub struct Analysis {
    pub slot: u64,
    pub parent: u64,
    pub index: u64,
    pub blockhash: String,
    pub raw: Vec<Value>,
    pub transactions: Vec<SemanticTransaction>,
}

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("HISTORICAL_BOUNDARY", message)
}

pub fn analyze(input: &HistoricalRequest, envelope: &Value) -> Result<Analysis, Error> {
    input.validate()?;
    if envelope.get("error").is_some() {
        return Err(invalid("RPC error is not an archived block"));
    }
    let block = &envelope["result"];
    let slot = input.candidate["slot"]
        .as_u64()
        .ok_or_else(|| invalid("missing target slot"))?;
    let parent = block["parentSlot"]
        .as_u64()
        .filter(|n| *n < slot)
        .ok_or_else(|| invalid("parent must precede target slot"))?;
    let index = input.candidate["transactionIndex"]
        .as_u64()
        .ok_or_else(|| invalid("missing target index"))?;
    let blockhash = block["blockhash"]
        .as_str()
        .ok_or_else(|| invalid("missing blockhash"))?
        .to_owned();
    address(&blockhash, 32)?;
    address(
        block["previousBlockhash"]
            .as_str()
            .ok_or_else(|| invalid("missing parent blockhash"))?,
        32,
    )?;
    let raw = block["transactions"]
        .as_array()
        .ok_or_else(|| invalid("full block transactions required"))?
        .clone();
    let mut transactions = Vec::with_capacity(raw.len());
    for (index, transaction) in raw.iter().enumerate() {
        instructions::resolve(transaction)?;
        transactions.push(summarize_semantic_transaction(transaction, index as u64)?);
    }
    let target = transactions
        .get(index as usize)
        .ok_or_else(|| invalid("target outside block"))?;
    if input.candidate["targetSignature"] != target.signature {
        return Err(invalid("candidate target signature differs"));
    }
    let source = input.candidate["blockSourceHash"]
        .as_str()
        .ok_or_else(|| invalid("missing raw source pin"))?;
    let hash = Digest::of(format!("{source}\n{index}\n{}\n", target.signature));
    if input.candidate["rawEvidenceHash"] != hash.as_str() {
        return Err(invalid("candidate transaction evidence pin differs"));
    }
    Ok(Analysis {
        slot,
        parent,
        index,
        blockhash,
        raw,
        transactions,
    })
}
