//! Persistent transport on a reviewed cached fixture; never fetches providers.
#[cfg(unix)]
mod unix {
    use serde_json::Value;
    use std::{path::PathBuf, time::Duration};
    use svm_replay_engine::shared::{
        historical::{NativeSession, SessionCommand},
        runtime::{
            file_sha256, CancellationToken, ExecutionBudget, ProcessOwner, WorkerLimits,
            WorkerSpec, WorkerTransport,
        },
    };

    pub(super) fn inputs() -> (Value, WorkerSpec, ProcessOwner) {
        let path =
            PathBuf::from(std::env::var_os("SVM_SESSION_FIXTURE").expect("reviewed fixture"));
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(
            svm_replay_protocol::Digest::of(&raw).as_str(),
            std::env::var("SVM_SESSION_FIXTURE_SHA256").unwrap()
        );
        let fixture: Value = serde_json::from_slice(&raw).unwrap();
        let worker = WorkerSpec {
            executable: PathBuf::from(
                std::env::var_os("SVM_SESSION_WORKER").expect("reviewed worker"),
            ),
            sha256: std::env::var("SVM_SESSION_WORKER_SHA256").expect("worker pin"),
        };
        let executable = PathBuf::from(env!("CARGO_BIN_EXE_svm-replay"));
        let owner = ProcessOwner {
            sha256: file_sha256(&executable).unwrap(),
            executable,
        };
        (fixture, worker, owner)
    }

    fn open(scratch: &std::path::Path, token: CancellationToken) -> NativeSession {
        let (fixture, worker, owner) = inputs();
        NativeSession::open(
            WorkerTransport::new(scratch).with_owner(owner),
            worker,
            fixture,
            true,
            WorkerLimits::default(),
            ExecutionBudget::new(Duration::from_secs(120), token).unwrap(),
        )
        .unwrap()
    }

    #[test]
    #[ignore = "requires reviewed SVM_SESSION_FIXTURE/WORKER pins; explicit cached integration driver"]
    fn real_cached_session_is_supervised_and_reaped() {
        let scratch = tempfile::tempdir().unwrap();
        let token = CancellationToken::new();
        let mut session = open(scratch.path(), token.clone());
        assert_eq!(session.ready()["kind"], "ready");
        let deadline = ExecutionBudget::new(Duration::from_secs(30), token).unwrap();
        let original = session.request(SessionCommand::State, &deadline).unwrap();
        let snapshot = session
            .request(SessionCommand::Snapshot, &deadline)
            .unwrap();
        session
            .request(
                SessionCommand::Select {
                    fork_id: snapshot["forkId"].as_u64().unwrap(),
                },
                &deadline,
            )
            .unwrap();
        let selected = session.request(SessionCommand::State, &deadline).unwrap();
        assert_eq!(original["stateSha256"], selected["stateSha256"]);
        let missing = session
            .request(
                SessionCommand::Accounts {
                    pubkeys: vec!["Vote111111111111111111111111111111111111111".into()],
                },
                &deadline,
            )
            .unwrap_err();
        assert_eq!(missing.code, "NEEDS_INPUT");
        assert_eq!(
            session.request(SessionCommand::State, &deadline).unwrap(),
            selected
        );
        let worker = session.close().unwrap();
        assert_eq!(worker.value["schema"], "svm-native-session-terminal/v1");
        assert_eq!(worker.value["closed"], true);
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
        #[cfg(target_os = "linux")]
        assert!(
            !PathBuf::from(format!("/proc/{}", worker.pid)).exists(),
            "owned worker reaped"
        );
    }

    #[test]
    #[ignore = "requires reviewed SVM_SESSION_FIXTURE/WORKER pins; explicit cached integration driver"]
    fn cancelled_session_is_joined_before_return_and_drop_leaves_no_scratch() {
        let scratch = tempfile::tempdir().unwrap();
        let token = CancellationToken::new();
        let mut session = open(scratch.path(), token.clone());
        token.cancel();
        let call = ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap();
        assert_eq!(
            session
                .request(SessionCommand::State, &call)
                .unwrap_err()
                .code,
            "WORKER_CANCELLED"
        );
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
        drop(session);
        let session = open(scratch.path(), CancellationToken::new());
        drop(session);
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
    }

    #[test]
    #[ignore = "requires reviewed SVM_SESSION_FIXTURE/WORKER pins; explicit cached integration driver"]
    fn bad_identity_and_unavailable_cold_scratch_fail_without_fallback() {
        let (fixture, mut worker, owner) = inputs();
        let scratch = tempfile::tempdir().unwrap();
        worker.sha256 = "0".repeat(64);
        let result = NativeSession::open(
            WorkerTransport::new(scratch.path()).with_owner(owner.clone()),
            worker,
            fixture.clone(),
            true,
            WorkerLimits::default(),
            ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap(),
        );
        let error = result.err().expect("invalid worker pin must fail");
        assert!(error
            .details
            .unwrap()
            .to_string()
            .contains("WORKER_INTEGRITY"));
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
        let (_, worker, _) = inputs();
        let result = NativeSession::open(
            WorkerTransport::new(scratch.path().join("unavailable")).with_owner(owner),
            worker,
            fixture,
            true,
            WorkerLimits::default(),
            ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).unwrap(),
        );
        assert!(result
            .err()
            .expect("missing cold scratch must fail")
            .details
            .unwrap()
            .to_string()
            .contains("create worker scratch"));
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
    }
}
#[cfg(unix)]
#[path = "native_session_initialization.rs"]
mod initialization;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "native_session_parent.rs"]
mod parent;
#[cfg(unix)]
#[path = "native_session_rpc.rs"]
mod rpc;
#[cfg(unix)]
#[path = "native_session_transactions.rs"]
mod transactions;
#[cfg(unix)]
#[path = "native_session_verified.rs"]
mod verified;
