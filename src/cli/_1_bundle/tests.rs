use super::*;

#[test]
fn bundle_permissions_preserve_read_only_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worker.exe");
    fs::write(&path, b"permission-test-not-a-runtime").unwrap();
    set_mode(&path, 0o500).unwrap();
    let meta = fs::metadata(&path).unwrap();
    assert!(meta.permissions().readonly());
    assert!(is_executable(&path, &meta));
    set_mode(&path, 0o600).unwrap();
    assert!(!fs::metadata(&path).unwrap().permissions().readonly());
    #[cfg(unix)]
    assert!(!is_executable(&path, &fs::metadata(&path).unwrap()));
}

#[test]
fn executable_names_follow_native_platform() {
    assert_eq!(
        binary_name("worker"),
        format!("worker{}", std::env::consts::EXE_SUFFIX)
    );
}
