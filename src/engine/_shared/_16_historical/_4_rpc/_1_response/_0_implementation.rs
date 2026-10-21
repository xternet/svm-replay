use super::*;

pub(crate) struct RpcReply {
    pub body: Option<Value>,
    pub close: bool,
}

#[derive(Debug)]
pub(in super::super) struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
    pub fn params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }
    pub fn response(self, id: Value) -> Value {
        let mut error = json!({"code":self.code,"message":self.message});
        if let Some(data) = self.data {
            error["data"] = data;
        }
        json!({"jsonrpc":"2.0","id":id,"error":error})
    }
}

impl From<Error> for RpcError {
    fn from(error: Error) -> Self {
        Self {
            code: if error.code == "NEEDS_INPUT" {
                -32010
            } else {
                -32000
            },
            message: error.message,
            data: Some(json!({"code":error.code,"details":error.details})),
        }
    }
}
