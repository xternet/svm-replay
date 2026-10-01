use super::*;

#[test]
fn paths_do_not_create_data_or_depend_on_checkout() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("new-data");
    assert_eq!(cache_dir(&data).unwrap(), data.join("cache"));
    assert_eq!(runs_dir(&data).unwrap(), data.join("runs"));
    assert!(!data.exists());
    assert_eq!(
        cache_dir(Path::new("relative")).unwrap_err().code,
        "DATA_DIRECTORY"
    );
}

#[test]
fn legacy_roots_are_rejected_without_modifying_old_evidence() {
    for name in ["index.sqlite", "blobs", "jobs"] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(name);
        fs::write(&path, b"preserved legacy evidence").unwrap();
        assert_eq!(
            cache_dir(root.path()).unwrap_err().code,
            "LEGACY_DATA_LAYOUT"
        );
        assert_eq!(
            runs_dir(root.path()).unwrap_err().code,
            "LEGACY_DATA_LAYOUT"
        );
        assert_eq!(fs::read(&path).unwrap(), b"preserved legacy evidence");
        assert!(!root.path().join("cache").exists());
        assert!(!root.path().join("runs").exists());
    }
}
