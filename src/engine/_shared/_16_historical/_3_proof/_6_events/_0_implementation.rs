use super::*;

impl VerifiedSession {
    /// Observe admitted local RPC transactions; no change to execution or worker identity.
    pub fn observe_transactions(&mut self, observer: impl FnMut(&Value) + Send + 'static) {
        self.observer = Some(Box::new(observer));
    }
}

pub(in super::super) fn transaction_event(
    command: &SessionCommand,
    response: &Value,
) -> Option<Value> {
    if !matches!(command, SessionCommand::RpcSend { .. }) || response["admitted"] != true {
        return None;
    }
    let output = &response["output"];
    Some(
        json!({"schema":"svm-historical-event/v1","kind":"transaction",
        "signature":response["signature"],"status":output["status"],
        "computeUnits":output["computeUnits"],"logs":output["logs"],
        "normalizedError":output["normalizedError"],"fee":output["fee"]}),
    )
}
