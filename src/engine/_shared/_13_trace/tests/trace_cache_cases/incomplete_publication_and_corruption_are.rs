use super::*;

#[test]
fn incomplete_publication_and_corruption_are_not_reused_or_silently_replaced() {
    let root = tempfile::tempdir().unwrap();
    let id = identity();
    let exported = export(&id);
    let origin = Digest::of(b"run");
    let mut source = Store::open(root.path()).unwrap();
    put_export(&mut source, &id, &origin, &exported).unwrap();
    let bytes = raw_get(&mut source, &id.key().unwrap());
    let manifest = parse_json(&bytes).unwrap();
    let payload_key: Digest =
        serde_json::from_value(manifest["payloadRef"]["key"].clone()).unwrap();
    let orphan = tempfile::tempdir().unwrap();
    let mut orphan_store = Store::open(orphan.path()).unwrap();
    raw_put(&mut orphan_store, &payload_key, &exported.payload);
    assert!(get_export(&mut orphan_store, &id).unwrap().is_none());
    let missing = tempfile::tempdir().unwrap();
    let mut missing_store = Store::open(missing.path()).unwrap();
    raw_put(&mut missing_store, &id.key().unwrap(), &bytes);
    assert_eq!(
        get_export(&mut missing_store, &id).err().unwrap().code,
        "CAPTURE_CACHE_CORRUPTION"
    );
    assert_eq!(
        put_export(&mut missing_store, &id, &origin, &exported)
            .unwrap_err()
            .code,
        "CAPTURE_CACHE_CORRUPTION"
    );
    let payload_hash = Digest::of(&exported.payload);
    std::fs::write(
        root.path().join("blobs").join(payload_hash.as_str()),
        b"corrupted",
    )
    .unwrap();
    assert_eq!(
        get_export(&mut source, &id).err().unwrap().code,
        "CAPTURE_CACHE_STORE_CORRUPTION"
    );
}

#[test]
fn manifest_and_payload_mutations_fail_closed() {
    let original_root = tempfile::tempdir().unwrap();
    let mut original = Store::open(original_root.path()).unwrap();
    let id = identity();
    let exported = export(&id);
    put_export(&mut original, &id, &Digest::of(b"run"), &exported).unwrap();
    let manifest = parse_json(&raw_get(&mut original, &id.key().unwrap())).unwrap();
    for (pointer, mutation) in [
        ("/schema", json!("unknown")),
        (
            "/identity/contextSha256",
            json!(Digest::of(b"wrong context")),
        ),
        ("/identitySha256", json!(Digest::of(b"wrong key"))),
        ("/originRunSha256", json!("bad hash")),
        ("/payloadRef/key", json!(Digest::of(b"wrong payload key"))),
        ("/payloadRef/sha256", json!(Digest::of(b"wrong digest"))),
        ("/payloadRef/bytes", json!(0)),
        ("/payloadRef/bytes", json!(268435457u64)),
        ("/artifact/status", json!("COMPLETE")),
        ("/artifact/payload/events", json!(2)),
        (
            "/artifact/identitySha256",
            json!(Digest::of(b"wrong artifact identity")),
        ),
        (
            "/artifact/payload/sha256",
            json!(Digest::of(b"wrong artifact payload")),
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(root.path()).unwrap();
        let mut changed = manifest.clone();
        *changed.pointer_mut(pointer).unwrap() = mutation;
        let key: Digest = serde_json::from_value(manifest["payloadRef"]["key"].clone()).unwrap();
        raw_put(&mut store, &key, &exported.payload);
        raw_put(
            &mut store,
            &id.key().unwrap(),
            &serde_json::to_vec(&changed).unwrap(),
        );
        assert_eq!(
            get_export(&mut store, &id).err().unwrap().code,
            "CAPTURE_CACHE_CORRUPTION",
            "{pointer}"
        );
    }
    for bytes in [
        b"{\"schema\":1,\"schema\":2}".to_vec(),
        {
            let mut value = manifest.clone();
            value["extra"] = json!(true);
            serde_json::to_vec(&value).unwrap()
        },
        {
            let mut value = manifest.clone();
            value.as_object_mut().unwrap().remove("payloadRef");
            serde_json::to_vec(&value).unwrap()
        },
        vec![b' '; 1024 * 1024 + 1],
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(root.path()).unwrap();
        raw_put(&mut store, &id.key().unwrap(), &bytes);
        assert_eq!(
            get_export(&mut store, &id).err().unwrap().code,
            "CAPTURE_CACHE_CORRUPTION"
        );
    }
}

#[test]
fn invalid_new_export_cannot_publish_a_manifest() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let id = identity();
    let mut bad = export(&id);
    bad.payload.push(b' ');
    assert!(put_export(&mut store, &id, &Digest::of(b"run"), &bad).is_err());
    assert!(get_export(&mut store, &id).unwrap().is_none());
}

#[test]
fn off_capture_preserves_absent_payload() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let mut id = identity();
    id.request.level = "off".into();
    let exported = svm_replay_engine::shared::trace::CaptureExport {
        artifact: json!({"schema":"svm-capture-artifact/v2","executionMode":"jit",
            "identitySha256":id.artifact_identity_sha256,"status":"OFF","reason":null,"payload":null,
            "coverage":{"calls":"none","sbpf":"none","accountData":"none","computeUnits":"none","nativeBuiltins":"none"}}),
        payload: Vec::new(),
    };
    assert!(exported.payload.is_empty());
    let receipt = put_export(&mut store, &id, &Digest::of(b"off run"), &exported).unwrap();
    assert_eq!(receipt.payload_sha256, None);
    let cached = get_export(&mut store, &id).unwrap().unwrap();
    assert_eq!(cached.export.artifact["status"], "OFF");
    assert!(cached.export.payload.is_empty());
}
