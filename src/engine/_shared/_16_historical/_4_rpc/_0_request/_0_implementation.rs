use super::*;

#[derive(Debug)]
pub(in super::super) struct Request {
    pub id: Option<Value>,
    pub method: String,
    pub params: Vec<Value>,
}

pub(in super::super) fn decode(value: &Value) -> Result<Request, RpcError> {
    let invalid = || RpcError::new(-32600, "Invalid Request");
    let object = value.as_object().ok_or_else(invalid)?;
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if object.get("jsonrpc") != Some(&json!("2.0"))
        || method.is_empty()
        || method.len() > 128
        || object
            .keys()
            .any(|key| !["jsonrpc", "id", "method", "params"].contains(&key.as_str()))
    {
        return Err(invalid());
    }
    let id = object.get("id").cloned();
    if id.as_ref().is_some_and(|id| {
        !(id.is_null()
            || id.is_i64()
            || id.is_u64()
            || id.as_str().is_some_and(|id| id.len() <= 128))
    }) {
        return Err(invalid());
    }
    // Omitted params means no arguments in JSON-RPC, not missing Bank evidence.
    let params = match object.get("params") {
        None => Vec::new(),
        Some(params) => params
            .as_array()
            .ok_or_else(|| RpcError::params("Solana RPC params must be positional"))?
            .clone(),
    };
    Ok(Request {
        id,
        method: method.into(),
        params,
    })
}
