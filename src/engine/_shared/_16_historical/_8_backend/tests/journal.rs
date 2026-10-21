use super::*;
use std::path::{Path, PathBuf};

/// A send in the embedder's store `revision`.
fn send(branch: u64, id: u64, wire: &str, revision: u64) -> String {
    format!(
        "{}\n",
        json!({"id":id,"branch":branch,"op":{"command":{"method":"send",
        "params":{"transactionBase64":wire,"sigVerify":false}}},"revision":revision})
    )
}

fn journal_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("m59-journal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("journal.jsonl")
}

fn serve(
    path: &Path,
    binding: &str,
    forks: &mut Forks,
    input: String,
) -> Result<Vec<Value>, Error> {
    serve_after(path, binding, "T", forks, input)
}

fn serve_after(
    path: &Path,
    binding: &str,
    after: &str,
    forks: &mut Forks,
    input: String,
) -> Result<Vec<Value>, Error> {
    let mut output = Vec::new();
    let journal = Some(JournalFile {
        path: path.to_path_buf(),
        binding: binding.to_string(),
        after: Some(after.to_string()),
    });
    _3_serve::serve_lines(
        std::io::Cursor::new(input),
        &mut output,
        forks,
        &budget(),
        |_| Ok(()),
        Some(json!({"kind":"ready"})),
        journal,
    )?;
    Ok(String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect())
}

/// Sends with a hydration, forks (outside any store revision), commits the fork back, and
/// reads (not journaled, so its revision 3 is not either).
fn session(path: &Path) -> String {
    let mut forks = Forks::new();
    forks.hydrate = vec!["h".into()];
    let input = [
        send(0, 1, "credit:a:5", 1),
        "{\"id\":2,\"op\":\"fork\"}\n".into(),
        send(1, 3, "credit:b:2", 2),
        "{\"id\":4,\"branch\":1,\"op\":{\"commit\":{\"into\":0}},\"revision\":2}\n".into(),
        "{\"id\":5,\"branch\":1,\"op\":{\"peek\":{\"pubkeys\":[\"b\"]}},\"revision\":3}\n".into(),
    ]
    .concat();
    let replies = serve(path, "db1", &mut forks, input).unwrap();
    let new = json!({"records":0,"state":null,"revision":null});
    assert_eq!(replies[0]["journal"], new);
    assert!(
        replies[1..]
            .iter()
            .all(|reply| reply.get("error").is_none()),
        "{replies:?}"
    );
    // Every reply says how many operations the journal holds once it is sent.
    let journaled: Vec<_> = replies[1..]
        .iter()
        .map(|r| r["journaled"].clone())
        .collect();
    assert_eq!(
        journaled,
        [json!(1), json!(2), json!(3), json!(4), json!(4)]
    );
    forks.state_digest(&budget()).unwrap()
}

#[test]
fn a_restart_replays_the_journal_to_the_same_state_with_its_recorded_hydrations() {
    let path = journal_path("replay");
    let state = session(&path);
    let records = std::fs::read_to_string(&path).unwrap().lines().count() - 1;
    assert_eq!(records, 4, "the peek is not journaled");

    let mut restarted = Forks::new();
    let replies = serve(&path, "db1", &mut restarted, String::new()).unwrap();
    let restored = json!({"records":4,"state":state,"revision":2});
    assert_eq!(
        replies[0]["journal"], restored,
        "the newest journaled store revision"
    );
    assert_eq!(restarted.state_digest(&budget()).unwrap(), state);
    assert!(
        restarted.hydrate.is_empty(),
        "replay acquired nothing itself"
    );
}

#[test]
fn a_journal_that_does_not_belong_or_does_not_reproduce_refuses_the_start() {
    let path = journal_path("refuse");
    session(&path);
    let original = std::fs::read_to_string(&path).unwrap();
    let start = |text: &str, binding: &str| {
        std::fs::write(&path, text).unwrap();
        serve(&path, binding, &mut Forks::new(), String::new())
            .unwrap_err()
            .code
    };
    assert_eq!(start(&original, "db2"), "JOURNAL_BINDING_MISMATCH");
    let other = original.replacen("\"slot\":100", "\"slot\":101", 1);
    assert_eq!(start(&other, "db1"), "JOURNAL_BOUNDARY_MISMATCH");
    std::fs::write(&path, &original).unwrap();
    let another_after = serve_after(&path, "db1", "U", &mut Forks::new(), String::new());
    assert_eq!(another_after.unwrap_err().code, "JOURNAL_BOUNDARY_MISMATCH");
    assert_eq!(start(original.trim_end(), "db1"), "JOURNAL_INCOMPLETE");
    let corrupt = format!("{original}not json\n");
    assert_eq!(start(&corrupt, "db1"), "JOURNAL_CORRUPT");
    let tampered = original.replacen("credit:b:2", "credit:b:3", 1);
    assert_eq!(start(&tampered, "db1"), "JOURNAL_DIVERGED");
}

#[test]
fn a_store_and_its_journal_must_agree_before_anything_is_prepared() {
    let path = journal_path("require");
    let code = |result: Result<(), Error>| result.unwrap_err().code;
    assert_eq!(
        code(require_journal(&path, Some("existing"))),
        "JOURNAL_MISSING"
    );
    require_journal(&path, Some("new")).unwrap();
    session(&path);
    assert_eq!(
        code(require_journal(&path, Some("new"))),
        "JOURNAL_STORE_MISMATCH"
    );
    require_journal(&path, Some("existing")).unwrap();
    // Another --after refuses before the journal's receipt is even looked at.
    let other = recorded_journal_input(&path, Some("U")).unwrap_err();
    assert_eq!(other.code, "JOURNAL_BOUNDARY_MISMATCH");
    let same = recorded_journal_input(&path, Some("T")).unwrap_err();
    assert_eq!(
        same.code, "JOURNAL_RECEIPT_MISSING",
        "the double retains no receipt"
    );
}

#[test]
fn a_journal_has_only_one_writer_and_releases_its_lease_on_drop() {
    let path = journal_path("writer");
    let descriptor = JournalFile {
        path: path.clone(),
        binding: "db1".into(),
        after: Some("T".into()),
    };
    let initial = Journal::open(
        &descriptor,
        &mut Branches::new(vec![]),
        &mut Forks::new(),
        &budget(),
    )
    .unwrap();
    drop(initial);
    let before = std::fs::read(&path).unwrap();
    let writer = Journal::open(
        &descriptor,
        &mut Branches::new(vec![]),
        &mut Forks::new(),
        &budget(),
    )
    .unwrap();
    let second = Journal::open(
        &descriptor,
        &mut Branches::new(vec![]),
        &mut Forks::new(),
        &budget(),
    );
    assert_eq!(
        second.err().expect("a competing writer must fail").code,
        "JOURNAL_IN_USE"
    );
    drop(writer);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let resumed = Journal::open(
        &descriptor,
        &mut Branches::new(vec![]),
        &mut Forks::new(),
        &budget(),
    )
    .unwrap();
    assert_eq!(resumed.records(), 0);
}
