use super::*;

#[test]
fn restart_roundtrip_and_namespace_isolation() {
    let dir = TempDir::new().expect("tempdir");
    assert_eq!(
        get(dir.path(), "raw", "historical slot 1")["status"],
        "MISS"
    );
    let stored = put(dir.path(), "raw", "historical slot 1", b"source bytes");
    assert_eq!(stored["status"], "STORED");
    let hit = get(dir.path(), "raw", "historical slot 1");
    assert_eq!(hit["status"], "HIT");
    assert_eq!(hit["sha256"], stored["sha256"]);
    assert_eq!(
        STANDARD
            .decode(hit["dataBase64"].as_str().expect("base64"))
            .expect("decode"),
        b"source bytes"
    );
    assert_eq!(
        get(dir.path(), "prepared", "historical slot 1")["status"],
        "MISS"
    );
    assert_eq!(
        put(dir.path(), "raw", "historical slot 1", b"source bytes")["status"],
        "STORED"
    );
    assert_eq!(
        put(dir.path(), "raw", "historical slot 1", b"conflict")["error"]["code"],
        "CONFLICT"
    );
}

#[test]
fn corruption_and_missing_blob_are_errors_never_misses() {
    let dir = TempDir::new().expect("tempdir");
    let value = put(dir.path(), "prepared", "k", b"checked snapshot");
    let path = dir
        .path()
        .join("blobs")
        .join(value["sha256"].as_str().expect("sha"));
    std::fs::write(&path, b"mutated snapshot").expect("tamper controlled file");
    assert_eq!(
        get(dir.path(), "prepared", "k")["error"]["code"],
        "CORRUPTION"
    );
    std::fs::remove_file(path).expect("remove exact test blob");
    assert_eq!(
        get(dir.path(), "prepared", "k")["error"]["code"],
        "CORRUPTION"
    );
}

#[test]
fn pins_leases_and_deduplicated_eviction_survive_restart() {
    let dir = TempDir::new().expect("tempdir");
    put(dir.path(), "raw", "a", b"shared");
    put(dir.path(), "prepared", "b", b"shared");
    put(dir.path(), "result", "c", b"other");
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"pin","namespace":"raw","key":"a","owner":"receipt-1"})
        )["status"],
        "OK"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"get","namespace":"result","key":"c","lease":{"owner":"job-1","expiresAtMs":4102444800000_u64}})
        )["status"],
        "HIT"
    );
    let evicted = call(dir.path(), json!({"op":"evict","maxBytes":0}));
    assert_eq!(evicted["limitSatisfied"], false);
    assert_eq!(evicted["remainingBytes"], 11);
    assert_eq!(get(dir.path(), "raw", "a")["status"], "HIT");
    assert_eq!(get(dir.path(), "prepared", "b")["status"], "MISS");
    assert_eq!(get(dir.path(), "result", "c")["status"], "HIT");
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"unpin","namespace":"raw","key":"a","owner":"wrong-owner"})
        )["error"]["code"],
        "NOT_OWNED"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"release","namespace":"result","key":"c","owner":"wrong-owner"})
        )["error"]["code"],
        "NOT_OWNED"
    );
    call(
        dir.path(),
        json!({"op":"unpin","namespace":"raw","key":"a","owner":"receipt-1"}),
    );
    call(
        dir.path(),
        json!({"op":"release","namespace":"result","key":"c","owner":"job-1"}),
    );
    assert_eq!(
        call(dir.path(), json!({"op":"evict","maxBytes":0}))["remainingBytes"],
        0
    );
}

#[test]
fn lru_eviction_order_is_deterministic() {
    let dir = TempDir::new().expect("tempdir");
    put(dir.path(), "raw", "a", b"aaaa");
    put(dir.path(), "raw", "b", b"bbbb");
    get(dir.path(), "raw", "a");
    let result = call(dir.path(), json!({"op":"evict","maxBytes":4}));
    assert_eq!(result["evictedEntries"], 1);
    assert_eq!(get(dir.path(), "raw", "b")["status"], "MISS");
    assert_eq!(get(dir.path(), "raw", "a")["status"], "HIT");
}

#[test]
fn cumulative_budgets_are_persisted_atomic_and_not_resettable() {
    let dir = TempDir::new().expect("tempdir");
    let limits = json!({"reads":2,"requests":3,"bytes":10});
    let open = json!({"op":"budget","action":"open","id":"job-1","limits":limits});
    assert_eq!(call(dir.path(), open.clone())["used"]["bytes"], 0);
    let charged = call(
        dir.path(),
        json!({"op":"budget","action":"charge","id":"job-1","amount":{"reads":1,"requests":1,"bytes":8}}),
    );
    assert_eq!(charged["used"]["bytes"], 8);
    assert_eq!(call(dir.path(), open)["used"]["bytes"], 8);
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"budget","action":"open","id":"job-1","limits":{"reads":999,"requests":999,"bytes":999}})
        )["error"]["code"],
        "BUDGET_CONFLICT"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"budget","action":"charge","id":"job-1","amount":{"reads":1,"requests":1,"bytes":3}})
        )["error"]["code"],
        "BUDGET_EXCEEDED"
    );
    let state = call(
        dir.path(),
        json!({"op":"budget","action":"inspect","id":"job-1"}),
    );
    assert_eq!(state["used"], json!({"reads":1,"requests":1,"bytes":8}));
}
