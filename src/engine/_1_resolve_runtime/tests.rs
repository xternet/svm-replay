use serde_json::json;
use std::fs;
use svm_replay_engine::_1_resolve_runtime::load_catalog;
use svm_replay_protocol::Digest;

fn catalog(file: &str, digest: Digest) -> serde_json::Value {
    json!({"schema":"svm-replay-workers/v1","platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"glibcMin":if cfg!(target_os="linux") { Some("2.38") } else { None }},"workers":[{
        "family":"v4-2","file":file,"sha256":digest,"buildHash":Digest::of(b"build"),"sourceSha256":Digest::of(b"source"),
        "executorSourceId":"test-runtime","capabilities":["guarded-generic-sysvars/v1"]}]})
}

#[test]
fn catalog_22_reference_requires_bytes_and_never_invents_capture() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("worker"), b"worker").unwrap();
    let path = temp.path().join("catalog.json");
    let mut value = catalog("worker", Digest::of(b"worker"));
    value["workers"][0]["family"] = json!("v2-2");
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    let loaded = load_catalog(&path, &Digest::of(&bytes)).unwrap();
    assert!(loaded.manifest.capture_workers.is_empty());
    fs::write(temp.path().join("worker"), b"changed").unwrap();
    assert_eq!(
        load_catalog(&path, &Digest::of(&bytes)).err().unwrap().code,
        "WORKER_IDENTITY"
    );
}
#[test]
fn catalog_rejects_foreign_architecture_before_opening_worker() {
    let temp = tempfile::tempdir().unwrap();
    let mut value = catalog("missing-worker", Digest::of(b"worker"));
    value["platform"]["arch"] = json!(if std::env::consts::ARCH == "x86_64" {
        "aarch64"
    } else {
        "x86_64"
    });
    let bytes = serde_json::to_vec(&value).unwrap();
    let path = temp.path().join("catalog.json");
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        load_catalog(&path, &Digest::of(&bytes)).err().unwrap().code,
        "UNSUPPORTED_PLATFORM"
    );
}
#[test]
fn catalog_requires_external_pin_and_confined_files() {
    let temp = tempfile::tempdir().unwrap();
    let worker = temp.path().join("worker");
    fs::write(&worker, b"worker").unwrap();
    let path = temp.path().join("catalog.json");
    let bytes = serde_json::to_vec(&catalog("worker", Digest::of(b"worker"))).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(load_catalog(&path, &Digest::of(&bytes)).is_ok());
    assert!(load_catalog(&path, &Digest::of(b"wrong")).is_err());
    let escape = serde_json::to_vec(&catalog("../worker", Digest::of(b"worker"))).unwrap();
    fs::write(&path, &escape).unwrap();
    assert!(load_catalog(&path, &Digest::of(&escape)).is_err());
}
#[test]
fn catalog_rejects_duplicate_family_and_worker_mutation() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("worker"), b"worker").unwrap();
    let path = temp.path().join("catalog.json");
    let mut value = catalog("worker", Digest::of(b"worker"));
    let first = value["workers"][0].clone();
    value["workers"].as_array_mut().unwrap().push(first);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(load_catalog(&path, &Digest::of(&bytes)).is_err());
    let bytes = serde_json::to_vec(&catalog("worker", Digest::of(b"wrong"))).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(load_catalog(&path, &Digest::of(&bytes)).is_err());
}

#[test]
fn optional_capture_role_requires_exact_reference_link_and_its_own_pin() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("worker"), b"worker").unwrap();
    fs::write(temp.path().join("capture"), b"capture").unwrap();
    let path = temp.path().join("catalog.json");
    let mut value = catalog("worker", Digest::of(b"worker"));
    let mut capture = value["workers"][0].clone();
    capture["file"] = json!("capture");
    capture["sha256"] = json!(Digest::of(b"capture"));
    capture["capabilities"] = json!([
        "guarded-generic-sysvars/v1",
        "runtime-call-journal/v1",
        "runtime-sbpf-trace/v1"
    ]);
    value["captureWorkers"] = json!([{"worker":capture,"referenceSha256":Digest::of(b"worker"),"gateSha256":Digest::of(b"externally reviewed gate") }]);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(load_catalog(&path, &Digest::of(&bytes)).is_ok());
    for (pointer, changed) in [
        (
            "/captureWorkers/0/referenceSha256",
            json!(Digest::of(b"other reference")),
        ),
        (
            "/captureWorkers/0/worker/sha256",
            json!(Digest::of(b"other capture")),
        ),
        (
            "/captureWorkers/0/worker/executorSourceId",
            json!("other-source"),
        ),
        ("/captureWorkers/0/gateSha256", json!("unreviewed")),
    ] {
        let mut bad = value.clone();
        *bad.pointer_mut(pointer).unwrap() = changed;
        let bytes = serde_json::to_vec(&bad).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(
            load_catalog(&path, &Digest::of(&bytes)).is_err(),
            "{pointer}"
        );
    }
    let mut duplicate = value.clone();
    duplicate["captureWorkers"]
        .as_array_mut()
        .unwrap()
        .push(value["captureWorkers"][0].clone());
    let bytes = serde_json::to_vec(&duplicate).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(load_catalog(&path, &Digest::of(&bytes)).is_err());
}
#[test]
fn source_binding_resolves_before_any_archive_or_provider_input_exists() {
    use svm_replay_engine::_1_resolve_runtime::resolve_binding;
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("worker"), b"worker").unwrap();
    let value = catalog("worker", Digest::of(b"worker"));
    let bytes = serde_json::to_vec(&value).unwrap();
    let path = temp.path().join("catalog.json");
    fs::write(&path, &bytes).unwrap();
    let installed = load_catalog(&path, &Digest::of(&bytes)).unwrap();
    let descriptor = &value["workers"][0];
    let mut binding = json!({"executor":{"id":descriptor["executorSourceId"],"sourceEvidenceHash":descriptor["sourceSha256"],
        "m9Build":{"binarySha256":descriptor["sha256"],"buildHash":descriptor["buildHash"],"capabilities":descriptor["capabilities"]}}});
    let profile = Digest::of(b"synthetic catalog profile");
    let candidate = json!({"executorSourceId":descriptor["executorSourceId"],"runtimeProfileId":profile,"slot":20});
    binding["schema"] = json!("svm-simulate-m6-runtime-binding/v1");
    binding["targetSlot"] = json!(20);
    binding["runtimeProfileId"] = json!(profile);
    binding["features"] = json!([]);
    binding["activeFeatureSetHash"] = json!(Digest::of(b"svm-feature-set/v1\n\n"));
    binding["enabledBuiltinsHash"] = json!(Digest::of(b"synthetic builtins"));
    binding["enabledPrecompilesHash"] = json!(Digest::of(b"synthetic precompiles"));
    binding["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&binding).unwrap()
    )));
    assert!(resolve_binding(&installed, "v4-2", &candidate, &binding).is_ok());
    binding["executor"]["m9Build"]["binarySha256"] = json!(Digest::of(b"different"));
    binding.as_object_mut().unwrap().remove("bindingHash");
    binding["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&binding).unwrap()
    )));
    assert_eq!(
        resolve_binding(&installed, "v4-2", &candidate, &binding)
            .err()
            .unwrap()
            .code,
        "WORKER_IDENTITY"
    );
}
