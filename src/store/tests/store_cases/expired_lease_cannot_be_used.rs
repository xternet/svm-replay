use super::*;

#[test]
fn expired_lease_cannot_be_used_and_no_longer_protects_entry() {
    let dir = TempDir::new().expect("tempdir");
    let lease = json!({"owner":"job-1","expiresAtMs":4102444800000_u64});
    call(
        dir.path(),
        json!({"op":"put","namespace":"raw","key":"k","dataBase64":STANDARD.encode(b"data"),"lease":lease}),
    );
    let db = rusqlite::Connection::open(dir.path().join("index.sqlite")).expect("test DB");
    db.execute("UPDATE leases SET expires=1", [])
        .expect("advance fixture deadline to expired");
    let result = call(
        dir.path(),
        json!({"op":"get","namespace":"raw","key":"k","lease":{"owner":"job-1","expiresAtMs":1}}),
    );
    assert_eq!(result["error"]["code"], "LEASE_EXPIRED");
    assert_eq!(
        call(dir.path(), json!({"op":"evict","maxBytes":0}))["remainingBytes"],
        0
    );
}

#[test]
fn blob_larger_than_old_64_mib_limit_roundtrips_in_real_process() {
    let dir = TempDir::new().expect("tempdir");
    let bytes: Vec<u8> = (0..104_500_000_usize)
        .map(|i| (i.wrapping_mul(31) % 251) as u8)
        .collect();
    let result = put(
        dir.path(),
        "prepared",
        "largest-current-fixture-envelope",
        &bytes,
    );
    assert_eq!(result["status"], "STORED");
    assert_eq!(result["sizeBytes"], bytes.len());
    let hit = get(dir.path(), "prepared", "largest-current-fixture-envelope");
    assert_eq!(hit["status"], "HIT");
    assert_eq!(
        STANDARD
            .decode(hit["dataBase64"].as_str().expect("base64"))
            .expect("decode"),
        bytes
    );
}
