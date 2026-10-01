use super::*;

#[test]
fn documented_trace_configuration_matches_public_contract() {
    let guide = include_str!("../../../../docs/tracing.md");
    let sample = guide
        .split("## Minimal capture configuration")
        .nth(1)
        .unwrap()
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let options: svm_replay_engine::TraceOptions = serde_json::from_str(sample).unwrap();
    options.capture.validate().unwrap();
    options.bounds.validate(&options.capture).unwrap();
}

#[test]
fn terminal_modes_preserve_script_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .args(["demo", "--json", "--example", "default"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "DEMO_NOT_INSTALLED");
    assert!(output.stderr.is_empty());
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .env("NO_COLOR", "1")
        .args(["demo", "--human", "--example", "default"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DEMO_NOT_INSTALLED"));
    assert!(!stdout.contains('\x1b'));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("Checking installed bundle"));
    let output = Command::new(env!("CARGO_BIN_EXE_svm-replay"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Usage:"));
}

#[test]
fn demo_without_installed_fixture_fails_clearly_without_network() {
    let (ok, response) = cli(&["demo"]);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "DEMO_NOT_INSTALLED");
}

#[test]
fn demo_distinguishes_absent_from_invalid_installation() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let exe = bin.join(if cfg!(windows) {
        "svm-replay.exe"
    } else {
        "svm-replay"
    });
    fs::copy(env!("CARGO_BIN_EXE_svm-replay"), &exe).unwrap();
    let run = || {
        let output = Command::new(&exe).arg("demo").output().unwrap();
        assert!(!output.status.success());
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    assert_eq!(run()["error"]["code"], "DEMO_NOT_INSTALLED");
    // An existing directory is invalid input, not an absent installation.
    fs::create_dir(dir.path().join("installation.json")).unwrap();
    assert_eq!(run()["error"]["code"], "DEMO_INPUT");
    fs::remove_dir(dir.path().join("installation.json")).unwrap();
    fs::write(dir.path().join("installation.json"), vec![b'x'; 4097]).unwrap();
    assert_eq!(run()["error"]["code"], "DEMO_INPUT");
}

#[test]
fn demo_rejects_wrong_bundle_pin_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("bundle.json");
    fs::write(&bundle, b"{}").unwrap();
    let (ok, response) = cli(&[
        "demo",
        "--bundle",
        bundle.to_str().unwrap(),
        "--bundle-sha256",
        &"0".repeat(64),
    ]);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "BUNDLE_INTEGRITY");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn validate_rejects_invalid_request_without_creating_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("request.json");
    fs::write(&path, b"{\"schema\":\"unknown\"}").unwrap();
    let (ok, response) = cli(&["validate", "--request", path.to_str().unwrap()]);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "INVALID_REQUEST");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
#[cfg(target_os = "linux")]
fn bundle_installs_offline_and_rejects_tampering_and_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let pin = catalog(dir.path());
    let input = dir.path().join("catalog.json");
    let bundle = dir.path().join("bundle");
    let (ok, response) = cli(&[
        "bundle",
        "pack",
        "--catalog",
        input.to_str().unwrap(),
        "--catalog-sha256",
        &pin,
        "--output",
        bundle.to_str().unwrap(),
    ]);
    assert!(ok, "{response}");
    let manifest = bundle.join("bundle.json");
    let bundle_pin = response["bundleSha256"].as_str().unwrap();
    assert_eq!(
        Digest::of(fs::read(&manifest).unwrap()).as_str(),
        bundle_pin
    );
    let installed = dir.path().join("installed");
    let args = [
        "bundle",
        "install",
        "--bundle",
        manifest.to_str().unwrap(),
        "--bundle-sha256",
        bundle_pin,
        "--output",
        installed.to_str().unwrap(),
    ];
    let (ok, result) = cli(&args);
    assert!(ok, "{result}");
    assert_eq!(
        fs::read(&manifest).unwrap(),
        fs::read(installed.join("bundle.json")).unwrap()
    );
    let (ok, result) = cli(&args);
    assert!(!ok);
    assert_eq!(result["error"]["code"], "INSTALL_EXISTS");
    let (ok, result) = cli(&[
        "capabilities",
        "--bundle",
        installed.join("bundle.json").to_str().unwrap(),
        "--bundle-sha256",
        bundle_pin,
    ]);
    assert!(ok, "{result}");
    let info: Value =
        serde_json::from_slice(&fs::read(installed.join("bundle.json")).unwrap()).unwrap();
    let worker = info["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "workers/v4-2-reference")
        .unwrap();
    assert!(worker["bytes"].as_u64().unwrap() > 0);
    let modified = installed.join("workers/v4-2-reference");
    fs::set_permissions(&modified, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&modified, b"changed").unwrap();
    let (ok, result) = cli(&[
        "doctor",
        "--bundle",
        installed.join("bundle.json").to_str().unwrap(),
        "--bundle-sha256",
        bundle_pin,
    ]);
    assert!(!ok);
    assert_eq!(result["error"]["code"], "BUNDLE_INTEGRITY");
}
