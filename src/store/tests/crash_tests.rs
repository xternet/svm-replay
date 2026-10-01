use super::*;
use std::process::Command;

#[test]
fn input_limits_reject_before_parsing_or_decoding() {
    let oversized = vec![0_u8; MAX_REQUEST_BYTES as usize + 1];
    assert_eq!(
        parse_request(&oversized).expect_err("oversized stdin").code,
        "INPUT_TOO_LARGE"
    );
    drop(oversized);
    let dir = tempfile::TempDir::new().expect("tempdir");
    let oversized_base64 = "A".repeat((MAX_BLOB_BYTES.div_ceil(3) * 4 + 1) as usize);
    let result = Store::open(dir.path()).expect("store").execute(json!({"version":1,"op":"put","namespace":"raw","key":"oversized","dataBase64":oversized_base64}));
    assert_eq!(result.expect_err("oversized blob").code, "INPUT_TOO_LARGE");
    assert_eq!(
        fs::read_dir(dir.path().join("blobs"))
            .expect("read blobs")
            .count(),
        0
    );
}

#[test]
#[ignore = "subprocess driver invoked only by crash recovery test"]
fn crash_child() {
    let root = std::env::var_os("SVM_REPLAY_STORE_TEST_ROOT").expect("test root");
    let mut store = Store::open(Path::new(&root)).expect("open actual database");
    let point = std::env::var("SVM_REPLAY_STORE_TEST_CRASH").expect("crash point");
    let request = if point.starts_with("eviction_") {
        json!({"version":1,"op":"evict","maxBytes":0})
    } else {
        json!({"version":1,"op":"put","namespace":"prepared","key":"target","dataBase64":STANDARD.encode(b"fully reconstructed boundary")})
    };
    store
        .execute(request)
        .expect("write must terminate at the test-only crash point");
    panic!("child missed the configured crash point");
}

fn kill_at(root: &Path, point: &str) {
    let output = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--ignored",
            "--exact",
            "crash_tests::crash_child",
            "--nocapture",
        ])
        .env("SVM_REPLAY_STORE_TEST_ROOT", root)
        .env("SVM_REPLAY_STORE_TEST_CRASH", point)
        .output()
        .expect("launch child");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            output.status.signal(),
            Some(9),
            "child must be SIGKILLed: {output:?}"
        );
    }
    #[cfg(windows)]
    assert_eq!(
        output.status.code(),
        Some(1),
        "child must be forcibly terminated: {output:?}"
    );
}

#[test]
fn killed_publication_never_exposes_partial_manifest() {
    for point in ["blob_durable", "manifest_uncommitted", "manifest_committed"] {
        let dir = tempfile::TempDir::new().expect("tempdir");
        Store::open(dir.path()).expect("initialize SQLite");
        kill_at(dir.path(), point);
        let mut reopened = Store::open(dir.path()).expect("restart");
        let result = reopened
            .execute(json!({"version":1,"op":"get","namespace":"prepared","key":"target"}))
            .expect("read after crash");
        assert_eq!(
            result["status"],
            if point == "manifest_committed" {
                "HIT"
            } else {
                "MISS"
            }
        );
        let evicted = reopened
            .execute(json!({"version":1,"op":"evict","maxBytes":0}))
            .expect("collect orphan");
        assert_eq!(evicted["removedBlobs"], 1);
        assert_eq!(
            fs::read_dir(dir.path().join("blobs"))
                .expect("read blobs")
                .count(),
            0
        );
    }
}

#[test]
fn killed_eviction_keeps_remaining_manifests_readable() {
    for point in ["eviction_uncommitted", "eviction_committed"] {
        let dir = tempfile::TempDir::new().expect("tempdir");
        Store::open(dir.path()).expect("store").execute(json!({"version":1,"op":"put","namespace":"raw","key":"entry","dataBase64":STANDARD.encode(b"actual data")})).expect("put");
        kill_at(dir.path(), point);
        let mut reopened = Store::open(dir.path()).expect("restart");
        let result = reopened
            .execute(json!({"version":1,"op":"get","namespace":"raw","key":"entry"}))
            .expect("no dangling manifest");
        assert_eq!(
            result["status"],
            if point == "eviction_uncommitted" {
                "HIT"
            } else {
                "MISS"
            }
        );
        reopened
            .execute(json!({"version":1,"op":"evict","maxBytes":0}))
            .expect("finish cleanup");
        assert_eq!(
            fs::read_dir(dir.path().join("blobs"))
                .expect("read blobs")
                .count(),
            0
        );
    }
}
