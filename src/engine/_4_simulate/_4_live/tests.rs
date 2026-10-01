use serde_json::json;
use std::path::PathBuf;
use svm_replay_engine::{
    debug_prepared,
    shared::{
        debug::channel,
        runtime::{CancellationToken, ProcessOwner},
    },
    {CacheMode, Config},
};
use svm_replay_protocol::{Digest, PreparedRequest};

#[test]
fn live_debug_requires_explicit_options_before_creating_a_job() {
    let root = tempfile::tempdir().unwrap();
    let config = Config {
        catalog_path: PathBuf::from("unread-catalog"),
        catalog_sha256: Digest::of(b"unread"),
        data_dir: root.path().join("not-created"),
        owner: ProcessOwner {
            executable: PathBuf::from("unstarted"),
            sha256: Digest::of(b"unstarted").as_str().into(),
        },
        cache: CacheMode::Off,
        trace: None,
    };
    // Structural request only: option validation must precede replay admission.
    let request: PreparedRequest = serde_json::from_value(json!({
        "schema":"svm-replay-prepared/v1","requestId":"options-only","family":"v3-0",
        "candidate":{},"fixture":{},"rawBlockBase64":"","blockSha256":Digest::of(b""),
        "sourceEvidenceHashes":[],"metadataPolicy":"STRICT",
        "limits":{"timeoutMs":1000,"maxOutputBytes":1024,"maxDiagnosticBytes":1024}
    }))
    .unwrap();
    let (_controller, mut driver) = channel();
    let error =
        debug_prepared(request, &config, CancellationToken::new(), &mut driver).unwrap_err();
    assert_eq!(error.code, "DEBUG_CONFIG");
    assert!(!config.data_dir.exists());
}
