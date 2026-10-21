use super::*;

#[test]
fn only_admitted_sends_emit_measured_native_fields() {
    let send = SessionCommand::RpcSend {
        transaction_base64: "wire".into(),
        skip_preflight: false,
    };
    let response = json!({"admitted":true,"signature":"signature","output":{
        "status":"err","computeUnits":123,"logs":["actual log"],"fee":"5000",
        "normalizedError":"{\"InstructionError\":[0,\"InvalidArgument\"]}"}});
    let event = transaction_event(&send, &response).unwrap();
    for key in ["status", "computeUnits", "logs", "fee", "normalizedError"] {
        assert_eq!(event[key], response["output"][key]);
    }
    assert_eq!(event["signature"], response["signature"]);
    assert!(transaction_event(&SessionCommand::State, &response).is_none());
    assert!(transaction_event(&send, &json!({"admitted":false})).is_none());
}
