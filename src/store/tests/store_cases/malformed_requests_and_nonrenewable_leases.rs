use super::*;

#[test]
fn malformed_requests_and_nonrenewable_leases_reject() {
    let dir = TempDir::new().expect("tempdir");
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"get","namespace":"raw","key":"k","unknown":true})
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(
        call(dir.path(), json!({"op":"evict","maxBytes":-1}))["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(
        call(dir.path(), json!({"op":"evict","maxBytes":1.5}))["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"evict","maxBytes":9007199254740992_u64})
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    put(dir.path(), "raw", "k", b"bytes");
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"lease","namespace":"raw","key":"k","owner":"job","expiresAtMs":1})
        )["error"]["code"],
        "LEASE_EXPIRED"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"lease","namespace":"raw","key":"k","owner":"job","expiresAtMs":4102444800000_u64})
        )["status"],
        "OK"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"lease","namespace":"raw","key":"k","owner":"job","expiresAtMs":4102444800001_u64})
        )["error"]["code"],
        "LEASE_CONFLICT"
    );
}

#[test]
fn actual_overrun_is_recorded_and_cannot_be_refunded() {
    let dir = TempDir::new().expect("tempdir");
    call(
        dir.path(),
        json!({"op":"budget","action":"open","id":"download","limits":{"reads":1,"requests":1,"bytes":4}}),
    );
    let observed = call(
        dir.path(),
        json!({"op":"budget","action":"observe","id":"download","amount":{"reads":0,"requests":0,"bytes":8}}),
    );
    assert_eq!(observed["error"]["code"], "BUDGET_EXCEEDED");
    assert_eq!(observed["details"]["used"]["bytes"], 8);
    assert_eq!(observed["details"]["limits"]["bytes"], 4);
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"budget","action":"inspect","id":"download"})
        )["used"]["bytes"],
        8
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"budget","action":"charge","id":"download","amount":{"reads":0,"requests":0,"bytes":0}})
        )["error"]["code"],
        "BUDGET_EXCEEDED"
    );
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"budget","action":"charge","id":"download","amount":{"reads":0,"requests":0,"bytes":-8}})
        )["error"]["code"],
        "INVALID_REQUEST"
    );
}

#[test]
fn concurrent_process_charges_never_oversubscribe() {
    let dir = TempDir::new().expect("tempdir");
    call(
        dir.path(),
        json!({"op":"budget","action":"open","id":"shared-job","limits":{"reads":10,"requests":10,"bytes":10}}),
    );
    let threads: Vec<_> = (0..14).map(|_| {
        let root = dir.path().to_path_buf();
        std::thread::spawn(move || call(&root, json!({"op":"budget","action":"charge","id":"shared-job","amount":{"reads":1,"requests":1,"bytes":1}})))
    }).collect();
    let mut passed = 0;
    let mut rejected = 0;
    for thread in threads {
        let value = thread.join().expect("thread");
        if value["status"] == "OK" {
            passed += 1;
        } else {
            assert_eq!(value["error"]["code"], "BUDGET_EXCEEDED");
            rejected += 1;
        }
    }
    assert_eq!((passed, rejected), (10, 4));
    assert_eq!(
        call(
            dir.path(),
            json!({"op":"budget","action":"inspect","id":"shared-job"})
        )["used"],
        json!({"reads":10,"requests":10,"bytes":10})
    );
}

#[test]
fn active_leases_survive_concurrent_publish_and_eviction() {
    let dir = TempDir::new().expect("tempdir");
    get(dir.path(), "raw", "initialize");
    let evict_root = dir.path().to_path_buf();
    let evictor = std::thread::spawn(move || {
        for _ in 0..30 {
            assert_eq!(
                call(&evict_root, json!({"op":"evict","maxBytes":0}))["status"],
                "OK"
            );
        }
    });
    let publishers: Vec<_> = (0..8).map(|i| {
        let root = dir.path().to_path_buf();
        std::thread::spawn(move || {
            let key = format!("boundary-{i}");
            let lease = json!({"owner":format!("job-{i}"),"expiresAtMs":4102444800000_u64});
            let result = call(&root, json!({"op":"put","namespace":"prepared","key":key,"dataBase64":STANDARD.encode(vec![i;32768]),"lease":lease}));
            assert_eq!(result["status"], "STORED");
            for _ in 0..3 {
                let read = call(&root, json!({"op":"get","namespace":"prepared","key":key,"lease":lease}));
                assert_eq!(read["status"], "HIT");
                assert_eq!(read["sha256"], result["sha256"]);
            }
        })
    }).collect();
    for publisher in publishers {
        publisher.join().expect("publisher");
    }
    evictor.join().expect("evictor");
    for i in 0..8 {
        assert_eq!(
            get(dir.path(), "prepared", &format!("boundary-{i}"))["status"],
            "HIT"
        );
    }
}

#[test]
fn released_lease_identity_cannot_be_reopened_after_eviction() {
    let dir = TempDir::new().expect("tempdir");
    let lease = json!({"owner":"job-1","expiresAtMs":4102444800000_u64});
    call(
        dir.path(),
        json!({"op":"put","namespace":"raw","key":"k","dataBase64":STANDARD.encode(b"data"),"lease":lease}),
    );
    call(
        dir.path(),
        json!({"op":"release","namespace":"raw","key":"k","owner":"job-1"}),
    );
    call(dir.path(), json!({"op":"evict","maxBytes":0}));
    let result = call(
        dir.path(),
        json!({"op":"put","namespace":"raw","key":"k","dataBase64":STANDARD.encode(b"data"),"lease":lease}),
    );
    assert_eq!(result["error"]["code"], "LEASE_CONFLICT");
    assert_eq!(get(dir.path(), "raw", "k")["status"], "MISS");
}
