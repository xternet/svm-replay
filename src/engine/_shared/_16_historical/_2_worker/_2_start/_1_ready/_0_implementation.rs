use super::*;

pub(in super::super) fn validate(ready: &Value, initialized: bool, slot: u64) -> Result<(), Error> {
    let common = ready["schema"] == "svm-native-session/v1"
        && ready["kind"] == "ready"
        && ready["state"].is_object();
    let control = if initialized {
        ready["original"].is_null()
            && ready.get("original").is_some()
            && ready["boundaryKind"] == "initialized-slot"
            && ready["originalControl"]["status"] == "NOT_APPLICABLE"
            && ready["state"]["slot"] == slot
            && ready["state"]["transactionIndex"] == 0
    } else {
        ready["original"].is_object()
            && ready
                .get("boundaryKind")
                .is_none_or(|kind| kind == "transaction")
    };
    if common && control {
        Ok(())
    } else {
        Err(error(
            "invalid native session ready frame or boundary control",
        ))
    }
}
