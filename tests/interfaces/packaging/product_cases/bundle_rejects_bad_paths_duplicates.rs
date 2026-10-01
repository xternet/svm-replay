use super::*;

#[test]
#[cfg(target_os = "linux")]
fn bundle_rejects_bad_paths_duplicates_pins_and_uninstalled_families() {
    let dir = tempfile::tempdir().unwrap();
    let pin = catalog(dir.path());
    let input = dir.path().join("catalog.json");
    let bundle = dir.path().join("bundle");
    let args = [
        "bundle",
        "pack",
        "--catalog",
        input.to_str().unwrap(),
        "--catalog-sha256",
        &pin,
        "--output",
        bundle.to_str().unwrap(),
    ];
    let (ok, response) = cli(&[args.as_slice(), &["--family", "v3-0"]].concat());
    assert!(!ok);
    assert_eq!(response["error"]["code"], "UNSUPPORTED_RUNTIME");
    assert!(!bundle.exists());
    let (ok, response) =
        cli(&[args.as_slice(), &["--family", "v4-2", "--reference-only"]].concat());
    assert!(ok, "{response}");
    let path = bundle.join("bundle.json");
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for bad in [
        "../escape",
        "/absolute",
        "bin//svm-replay",
        "bin/./svm-replay",
        "bin/../svm-replay",
        "bin/",
    ] {
        let mut m = original.clone();
        m["files"][0]["path"] = json!(bad);
        let bytes = serde_json::to_vec(&m).unwrap();
        fs::write(&path, &bytes).unwrap();
        let (ok, response) = cli(&[
            "doctor",
            "--bundle",
            path.to_str().unwrap(),
            "--bundle-sha256",
            Digest::of(bytes).as_str(),
        ]);
        assert!(!ok);
        assert_eq!(
            response["error"]["code"], "BUNDLE_PATH",
            "{bad}: {response}"
        );
    }
    let mut duplicate = original.clone();
    let first = duplicate["files"][0].clone();
    duplicate["files"].as_array_mut().unwrap().push(first);
    let bytes = serde_json::to_vec(&duplicate).unwrap();
    fs::write(&path, &bytes).unwrap();
    let (ok, response) = cli(&[
        "doctor",
        "--bundle",
        path.to_str().unwrap(),
        "--bundle-sha256",
        Digest::of(bytes).as_str(),
    ]);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "BUNDLE_FORMAT");
    fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
    let (ok, response) = cli(&[
        "doctor",
        "--bundle",
        path.to_str().unwrap(),
        "--bundle-sha256",
        Digest::of(b"wrong independent pin").as_str(),
    ]);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "BUNDLE_INTEGRITY");
    let executable = bundle.join("bin/svm-replay");
    // Replace only this test-owned copy with a symlink to the original test binary.
    fs::remove_file(&executable).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_svm-replay"), &executable).unwrap();
    let (ok, response) = cli(&[
        "doctor",
        "--bundle",
        path.to_str().unwrap(),
        "--bundle-sha256",
        response_pin(&original).as_str(),
    ]);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "BUNDLE_PATH");
}

#[test]
fn request_builder_requires_reviewed_evidence_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let feature = "11111111111111111111111111111111";
    let profile = Digest::of(b"CLI builder synthetic profile");
    let candidate = json!({"slot":20,"transactionIndex":0,"blockSourceHash":Digest::of(b"block"),
        "runtimeProfileId":profile,"executorSourceId":"executor","litesvmCommit":"commit","activeFeatures":[feature]});
    let mut binding = json!({"schema":"svm-simulate-m6-runtime-binding/v1","targetSlot":20,"runtimeProfileId":profile,
        "executor":{"id":"executor","litesvmCommit":"commit"},"features":[{"id":feature,"activationSlot":19}],
        "activeFeatureSetHash":Digest::of(format!("svm-feature-set/v1\n{feature}\n")),
        "enabledBuiltinsHash":Digest::of(b"builtins"),"enabledPrecompilesHash":Digest::of(b"precompiles")});
    binding["bindingHash"] = json!(Digest::of(format!(
        "m6-runtime-binding/v1\n{}\n",
        serde_json::to_string(&binding).unwrap()
    )));
    let c = dir.path().join("candidate.json");
    let b = dir.path().join("binding.json");
    let out = dir.path().join("request.json");
    fs::write(&c, serde_json::to_vec(&candidate).unwrap()).unwrap();
    fs::write(&b, serde_json::to_vec(&binding).unwrap()).unwrap();
    let args = [
        "request",
        "--candidate",
        c.to_str().unwrap(),
        "--runtime-binding",
        b.to_str().unwrap(),
        "--family",
        "v4-2",
        "--genesis-hash",
        feature,
        "--request-id",
        "builder-test",
        "--output",
        out.to_str().unwrap(),
    ];
    let (ok, response) = cli(&args);
    assert!(ok, "{response}");
    assert_eq!(response["metadataPolicy"], "STRICT");
    let bytes = fs::read(&out).unwrap();
    assert_eq!(response["sha256"], Digest::of(&bytes).as_str());
    let (ok, response) = cli(&["validate", "--request", out.to_str().unwrap()]);
    assert!(ok, "{response}");
    assert_eq!(response["executionPerformed"], false);
    assert_eq!(response["requestId"], "builder-test");
    let (ok, response) = cli(&args);
    assert!(!ok);
    assert_eq!(response["error"]["code"], "REQUEST_IO");
    assert_eq!(fs::read(&out).unwrap(), bytes);
}
