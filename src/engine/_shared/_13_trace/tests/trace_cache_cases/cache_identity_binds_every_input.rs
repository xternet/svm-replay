use super::*;

#[test]
fn cache_identity_binds_every_input_and_normalizes_order_only() {
    let original = identity();
    let key = original.key().unwrap();
    let mut reversed = original.clone();
    reversed.worker.capabilities.reverse();
    assert_eq!(key, reversed.key().unwrap());
    let fields = [
        "inputSha256",
        "contextSha256",
        "artifactIdentitySha256",
        "worker/sha256",
        "worker/buildHash",
        "worker/sourceSha256",
        "worker/file",
        "worker/executorSourceId",
        "worker/family",
        "request/limits/maxBytes",
        "request/limits/maxEvents",
        "request/limits/timeoutMs",
        "producerBounds/maxInvocations",
        "request/executionMode",
        "request/filter/instructionIndices",
        "request/filter/programIds",
        "worker/capabilities",
    ];
    for field in fields {
        let mut changed = serde_json::to_value(&original).unwrap();
        let value = match field {
            "worker/file" | "worker/executorSourceId" | "worker/family" => json!("changed"),
            "request/executionMode" => json!("interpreter-debug"),
            "request/filter/instructionIndices" => json!([1]),
            "request/filter/programIds" => json!(["11111111111111111111111111111111"]),
            "worker/capabilities" => json!(["runtime-call-journal/v1"]),
            field if field.starts_with("request/limits/") => json!(100),
            "producerBounds/maxInvocations" => json!(9),
            _ => json!(Digest::of(b"different")),
        };
        *changed.pointer_mut(&format!("/{field}")).unwrap() = value;
        let changed: ExportCacheIdentity = serde_json::from_value(changed).unwrap();
        assert_ne!(key, changed.key().unwrap(), "{field}");
    }
    let mut filters = original.clone();
    filters.request.filter.instruction_indices = vec![3, 1];
    let sorted_key = filters.key().unwrap();
    filters.request.filter.instruction_indices.reverse();
    assert_eq!(sorted_key, filters.key().unwrap());
    let mut sbpf = original.clone();
    sbpf.request.level = "sbpf".into();
    sbpf.request.sbpf_observations = "pc-registers".into();
    sbpf.producer_bounds.register_rows = 4;
    let sbpf_key = sbpf.key().unwrap();
    assert_ne!(key, sbpf_key);
    sbpf.producer_bounds.register_rows += 1;
    assert_ne!(sbpf_key, sbpf.key().unwrap());
    sbpf.request.sbpf_observations = "pc-registers-memory".into();
    sbpf.producer_bounds.memory_rows = Some(4);
    let memory_key = sbpf.key().unwrap();
    assert_ne!(sbpf_key, memory_key);
    sbpf.producer_bounds.memory_rows = Some(5);
    assert_ne!(memory_key, sbpf.key().unwrap());
}

#[test]
fn publish_reopen_and_idempotence_preserve_truncation_bytes_and_origin() {
    let root = tempfile::tempdir().unwrap();
    let id = identity();
    let exported = export(&id);
    assert_eq!(exported.artifact["status"], "TRUNCATED");
    let origin = Digest::of(b"original verified run");
    let mut store = Store::open(root.path()).unwrap();
    assert!(get_export(&mut store, &id).unwrap().is_none());
    let receipt = put_export(&mut store, &id, &origin, &exported).unwrap();
    assert!(!receipt.reused && !receipt.new_execution);
    let manifest_bytes = raw_get(&mut store, &id.key().unwrap());
    let manifest = parse_json(&manifest_bytes).unwrap();
    assert!(manifest_bytes.len() < 1024 * 1024);
    assert!(manifest.get("payloadBase64").is_none());
    let payload_key = serde_json::from_value(manifest["payloadRef"]["key"].clone()).unwrap();
    assert_eq!(raw_get(&mut store, &payload_key), exported.payload);
    drop(store);
    let mut reopened = Store::open(root.path()).unwrap();
    let cached = get_export(&mut reopened, &id).unwrap().unwrap();
    assert_eq!(cached.export.artifact, exported.artifact);
    assert_eq!(cached.export.payload, exported.payload);
    assert!(cached.receipt.reused && !cached.receipt.new_execution);
    assert_eq!(cached.receipt.origin_run_sha256, origin);
    let repeat = put_export(
        &mut reopened,
        &id,
        &Digest::of(b"later identical run"),
        &exported,
    )
    .unwrap();
    assert!(repeat.reused && !repeat.new_execution);
    assert_eq!(repeat.origin_run_sha256, origin);
    assert_eq!(raw_get(&mut reopened, &id.key().unwrap()), manifest_bytes);
    let mut changed = export(&id);
    changed.artifact["reason"] = json!("different producer limit evidence");
    assert_eq!(
        put_export(&mut reopened, &id, &origin, &changed)
            .unwrap_err()
            .code,
        "CAPTURE_CACHE_CONFLICT"
    );
    assert_eq!(raw_get(&mut reopened, &id.key().unwrap()), manifest_bytes);
    changed.payload = br#"[{"kind":"explicit synthetic alternate observation"}]"#.to_vec();
    changed.artifact["payload"]["bytes"] = json!(changed.payload.len());
    changed.artifact["payload"]["sha256"] = json!(Digest::of(&changed.payload));
    assert_eq!(
        put_export(&mut reopened, &id, &origin, &changed)
            .unwrap_err()
            .code,
        "CAPTURE_CACHE_CONFLICT"
    );
}

#[test]
fn real_store_publication_faults_leave_no_discoverable_manifest() {
    let id = identity();
    let exported = export(&id);
    let origin = Digest::of(b"publication fault test run");
    let original_root = tempfile::tempdir().unwrap();
    let mut original = Store::open(original_root.path()).unwrap();
    put_export(&mut original, &id, &origin, &exported).unwrap();
    let manifest_bytes = raw_get(&mut original, &id.key().unwrap());
    let manifest = parse_json(&manifest_bytes).unwrap();
    let payload_key: Digest =
        serde_json::from_value(manifest["payloadRef"]["key"].clone()).unwrap();
    for (fault_at_manifest, hash) in [
        (false, Digest::of(&exported.payload)),
        (true, Digest::of(&manifest_bytes)),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(root.path()).unwrap();
        // A real corrupt pre-existing blob makes Store's create-only publisher fail.
        std::fs::write(
            root.path().join("blobs").join(hash.as_str()),
            b"explicit corruption",
        )
        .unwrap();
        assert_eq!(
            put_export(&mut store, &id, &origin, &exported)
                .unwrap_err()
                .code,
            "CAPTURE_CACHE_STORE_CORRUPTION"
        );
        assert!(get_export(&mut store, &id).unwrap().is_none());
        let payload = store
            .execute(json!({"version":1,"op":"get","namespace":"raw","key":payload_key}))
            .unwrap();
        assert_eq!(
            payload["status"],
            if fault_at_manifest { "HIT" } else { "MISS" }
        );
    }
}
