use super::*;

#[test]
#[ignore = "requires pinned retained export/session/run paths and a new approved work root; no workers or network"]
fn retained_large_export_reopens_byte_exact_without_execution() {
    use std::{env, fs, path::PathBuf, time::Instant};
    use svm_replay_engine::shared::runtime::{file_sha256, read_bounded_file};
    let started = Instant::now();
    let input = |name: &str| env::var(format!("SVM_REPLAY_TEST_TRACE_CACHE_{name}")).expect(name);
    let export_dir = PathBuf::from(input("EXPORT_DIR"));
    let session_dir = PathBuf::from(input("SESSION_DIR"));
    let session_sha = Digest::new(input("SESSION_SHA256")).unwrap();
    let export_receipt_sha = Digest::new(input("EXPORT_RECEIPT_SHA256")).unwrap();
    let session_bytes = read_bounded_file(&session_dir.join("manifest.json"), 1024 * 1024).unwrap();
    assert_eq!(Digest::of(&session_bytes), session_sha);
    let session = parse_json(&session_bytes).unwrap();
    assert_eq!(session["schema"], "m14-saved-session/v1");
    for (name, pin) in session["files"].as_object().unwrap() {
        assert_eq!(std::path::Path::new(name).components().count(), 1);
        let path = session_dir.join(name);
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            pin["bytes"].as_u64().unwrap()
        );
        assert_eq!(json!(file_sha256(&path).unwrap()), pin["sha256"]);
    }
    let retained_receipt =
        read_bounded_file(&export_dir.join("receipt.json"), 1024 * 1024).unwrap();
    assert_eq!(Digest::of(&retained_receipt), export_receipt_sha);
    let retained = parse_json(&retained_receipt).unwrap();
    assert_eq!(retained["schema"], "m13-session-export/v2");
    assert_eq!(retained["session"]["manifestSha256"], json!(session_sha));
    assert_eq!(
        retained["session"]["workerSha256"],
        session["worker"]["sha256"]
    );
    assert_eq!(retained["sessionIdentity"], session["sessionIdentity"]);
    let origin: Digest = serde_json::from_value(retained["runManifestSha256"].clone()).unwrap();
    assert_eq!(
        file_sha256(&PathBuf::from(input("RUN_MANIFEST"))).unwrap(),
        origin.as_str()
    );
    let build =
        parse_json(&read_bounded_file(&session_dir.join("build.json"), 1024 * 1024).unwrap())
            .unwrap();
    let mut worker = session["worker"].clone();
    worker["file"] = json!("worker");
    worker["sourceSha256"] = build["source"]["sha256"].clone();
    assert_eq!(worker["sha256"], session["files"]["worker"]["sha256"]);
    assert_eq!(
        worker["buildHash"],
        session["files"]["build.json"]["sha256"]
    );
    let artifact =
        parse_json(&read_bounded_file(&export_dir.join("artifact.json"), 1024 * 1024).unwrap())
            .unwrap();
    assert_eq!(artifact, retained["artifact"]);
    let id = ExportCacheIdentity {
        input_sha256: serde_json::from_value(session["files"]["fixture.json"]["sha256"].clone())
            .unwrap(),
        worker: serde_json::from_value(worker).unwrap(),
        request: serde_json::from_value(session["request"].clone()).unwrap(),
        producer_bounds: serde_json::from_value(session["bounds"].clone()).unwrap(),
        context_sha256: session_sha,
        artifact_identity_sha256: serde_json::from_value(artifact["identitySha256"].clone())
            .unwrap(),
    };
    let payload = read_bounded_file(&export_dir.join("events.json"), 256 * 1024 * 1024).unwrap();
    assert_eq!(json!(payload.len()), retained["payloadRef"]["bytes"]);
    assert_eq!(
        json!(Digest::of(&payload)),
        retained["payloadRef"]["sha256"]
    );
    let payload_sha = Digest::of(&payload);
    let payload_bytes = payload.len();
    let work = tempfile::Builder::new()
        .prefix("trace-cache-reopen-")
        .tempdir_in(PathBuf::from(input("WORK_ROOT")))
        .unwrap()
        .keep();
    let mut store = Store::open(&work.join("store")).unwrap();
    let export = svm_replay_engine::shared::trace::CaptureExport {
        artifact: artifact.clone(),
        payload,
    };
    let inserted = put_export(&mut store, &id, &origin, &export).unwrap();
    assert!(!inserted.reused && !inserted.new_execution);
    drop(export);
    let manifest_bytes = raw_get(&mut store, &id.key().unwrap()).len();
    assert!(manifest_bytes < 1024 * 1024);
    drop(store);
    let mut store = Store::open(&work.join("store")).unwrap();
    let reopened = get_export(&mut store, &id).unwrap().unwrap();
    assert_eq!(reopened.export.artifact, artifact);
    assert_eq!(Digest::of(&reopened.export.payload), payload_sha);
    assert_eq!(reopened.export.payload.len(), payload_bytes);
    assert!(reopened.receipt.reused && !reopened.receipt.new_execution);
    assert_eq!(reopened.receipt.origin_run_sha256, origin);
    assert_eq!(
        file_sha256(&export_dir.join("events.json")).unwrap(),
        payload_sha.as_str()
    );
    let summary = json!({"schema":"svm-m17-retained-trace-cache-test/v1","status":"PASS",
        "scope":"pinned-retained-export-storage-and-reopen; no new execution or historical certification",
        "exportDirectory":export_dir,"exportReceiptSha256":export_receipt_sha,"sessionDirectory":session_dir,
        "identity":id,"artifact":artifact,"payloadBytes":payload_bytes,"payloadSha256":payload_sha,
        "cacheManifestBytes":manifest_bytes,"putReceipt":inserted,"reopenReceipt":reopened.receipt,
        "wallSeconds":started.elapsed().as_secs_f64()});
    fs::write(
        work.join("receipt.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("{}", work.join("receipt.json").display());
}
