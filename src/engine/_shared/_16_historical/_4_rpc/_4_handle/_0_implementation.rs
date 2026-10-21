use super::*;

fn one(session: &mut VerifiedSession, value: &Value, budget: &ExecutionBudget) -> RpcReply {
    let request = match _0_request::decode(value) {
        Ok(request) => request,
        Err(error) => {
            let invalid_params = error.code == -32602;
            let body = if invalid_params && value.get("id").is_none() {
                None
            } else {
                Some(error.response(if invalid_params {
                    value["id"].clone()
                } else {
                    Value::Null
                }))
            };
            return RpcReply { body, close: false };
        }
    };
    let result = _3_dispatch::dispatch(session, &request, budget);
    let (value, close) = match result {
        Ok((value, close)) => (
            json!({"jsonrpc":"2.0","id":request.id,"result":value}),
            close,
        ),
        Err(error) => {
            let id = match request.id.clone() {
                Some(id) => id,
                None => Value::Null,
            };
            (error.response(id), false)
        }
    };
    RpcReply {
        body: request.id.map(|_| value),
        close,
    }
}

impl VerifiedSession {
    pub(crate) fn rpc(&mut self, value: &Value, budget: &ExecutionBudget) -> RpcReply {
        let Some(batch) = value.as_array() else {
            return one(self, value, budget);
        };
        if batch.is_empty() || batch.len() > 64 {
            return RpcReply {
                body: Some(
                    RpcError::new(-32600, "batch must contain 1 to 64 requests")
                        .response(Value::Null),
                ),
                close: false,
            };
        }
        let mut responses = Vec::new();
        let mut close = false;
        for value in batch {
            let reply = one(self, value, budget);
            if let Some(body) = reply.body {
                responses.push(body);
            }
            close |= reply.close;
        }
        RpcReply {
            body: if responses.is_empty() {
                None
            } else {
                Some(json!(responses))
            },
            close,
        }
    }
}
