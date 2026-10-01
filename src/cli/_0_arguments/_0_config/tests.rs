use super::*;
#[test]
fn explicit_absolute_directory_is_not_rebased_or_created() {
    let parent = tempfile::tempdir().unwrap();
    let path = parent.path().join("data");
    assert_eq!(data_directory(Some(path.clone())).unwrap(), path);
    assert!(!path.exists());
}
#[test]
fn relative_directory_resolves_at_the_cli_boundary() {
    let relative = PathBuf::from(".svm-replay");
    assert_eq!(
        data_directory(Some(relative.clone())).unwrap(),
        std::env::current_dir().unwrap().join(relative)
    );
}
