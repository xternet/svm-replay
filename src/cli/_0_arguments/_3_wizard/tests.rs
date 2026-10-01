use super::*;
#[test]
fn wizard_uses_existing_parser_without_shell_interpolation() {
    let argv = [
        "svm-replay",
        "simulate",
        "--human",
        "--request",
        "a file; echo nope.json",
        "--catalog",
        "catalog.json",
        "--catalog-sha256",
        &"a".repeat(64),
        "--cache",
        "prepared",
    ]
    .map(String::from)
    .to_vec();
    match command(argv).unwrap() {
        Command::Simulate {
            request,
            runtime,
            cache: CacheChoice::Prepared,
            alchemy: None,
            ..
        } => {
            assert_eq!(request, PathBuf::from("a file; echo nope.json"));
            assert_eq!(runtime.catalog, Some(PathBuf::from("catalog.json")));
        }
        _ => panic!("wrong wizard command"),
    }
}
#[test]
fn command_preview_quotes_apostrophes() {
    let quoted = quote("a'b");
    assert_eq!(
        quoted,
        if cfg!(windows) {
            "'a''b'"
        } else {
            "'a'\"'\"'b'"
        }
    );
}
