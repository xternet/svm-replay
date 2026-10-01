use serde_json::{json, Value};
use svm_replay_protocol::{runtime::validate_runtime_binding, Digest};
fn binding() -> Value {
    let feature = "11111111111111111111111111111111";
    let mut value = json!({"schema":"svm-simulate-m6-runtime-binding/v1","targetSlot":20,
        "runtimeProfileId":Digest::of(b"explicit reviewed synthetic profile"),
        "executor":{"id":"test"},"features":[{"id":feature,"activationSlot":19}],
        "activeFeatureSetHash":Digest::of(format!("svm-feature-set/v1\n{feature}\n")),
        "enabledBuiltinsHash":Digest::of(b"builtins"),"enabledPrecompilesHash":Digest::of(b"precompiles")});
    value["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&value).unwrap()
    )));
    value
}
#[test]
fn binding_seal_covers_features_activations_executor_and_boundary() {
    let original = binding();
    validate_runtime_binding(&original).unwrap();
    for (pointer, value) in [
        ("/targetSlot", json!(21)),
        ("/features/0/activationSlot", json!(18)),
        ("/executor/id", json!("other")),
        (
            "/features/0/id",
            json!("SysvarC1ock11111111111111111111111111111111"),
        ),
        ("/bindingHash", json!(Digest::of(b"wrong"))),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(validate_runtime_binding(&bad).is_err(), "{pointer}");
    }
    let mut bad = original.clone();
    bad["features"][0]["activationSlot"] = json!(21);
    bad.as_object_mut().unwrap().remove("bindingHash");
    bad["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&bad).unwrap()
    )));
    assert!(
        validate_runtime_binding(&bad).is_err(),
        "future activation cannot be fixed by resealing"
    );
}

#[test]
fn feature_hash_cannot_lie_even_with_recomputed_binding_seal() {
    let mut bad = binding();
    bad["activeFeatureSetHash"] = json!(Digest::of(b"wrong features"));
    bad.as_object_mut().unwrap().remove("bindingHash");
    bad["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&bad).unwrap()
    )));
    assert!(validate_runtime_binding(&bad).is_err());
}
