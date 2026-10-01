use super::*;
use std::process::{Command, Stdio};

#[test]
fn blocked_output_fixture() {
    if std::env::var_os("SVM_REPLAY_TEST_BLOCKED_OUTPUT").is_none() {
        return;
    }
    write_final(&Value::String("x".repeat(1024 * 1024))).expect_err("blocked pipe must time out");
    // The libtest summary would itself write to the deliberately full pipe.
    // The production writer has already been joined before this helper exits.
    std::process::exit(0);
}

#[test]
fn blocked_stdout_cancels_and_joins_writer() {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "debugger::windows::tests::blocked_output_fixture",
            "--nocapture",
        ])
        .env("SVM_REPLAY_TEST_BLOCKED_OUTPUT", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "bounded output helper failed: {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate blocked fixture");
            child.wait().expect("reap blocked fixture");
            panic!("Windows stdout cancellation failed to release the writer");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
