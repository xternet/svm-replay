#![cfg(any(target_os = "linux", target_os = "macos"))]

use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command, thread, time::Duration};
use svm_replay_engine::shared::runtime::{
    file_sha256, CancellationToken, CleanupScope, ExecutionBudget, ProcessOwner, WorkerErrorCode,
    WorkerLimits, WorkerSpec, WorkerTransport,
};

fn worker(root: &Path, body: &str) -> WorkerSpec {
    let executable = root.join("worker");
    fs::write(&executable, format!("#!/bin/sh\n{body}\n")).expect("worker fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).expect("fixture mode");
    WorkerSpec {
        sha256: file_sha256(&executable).expect("fixture pin"),
        executable,
    }
}

fn transport(root: &Path) -> WorkerTransport {
    // Cargo can replace its output while another test tranche builds the CLI.
    // Pin this test's own opened/copied executable, not the shared build target.
    let executable = root.join("worker-owner");
    fs::copy(env!("CARGO_BIN_EXE_svm-replay"), &executable).expect("copy immutable owner fixture");
    let sha256 = file_sha256(&executable).expect("owner pin");
    WorkerTransport::new(root).with_owner(ProcessOwner { executable, sha256 })
}

#[test]
fn owner_reaps_descendants_after_worker_exits_normally() {
    let root = tempfile::tempdir().expect("test root");
    let pid_file = root.path().join("descendant.pid");
    let spec = worker(
        root.path(),
        "/bin/sleep 30 & descendant=$!; printf '%s' \"$descendant\" > \"$3\"; printf '{}' > \"$2\"",
    );
    let setup_started = std::time::Instant::now();
    let transport = transport(root.path());
    eprintln!(
        "immutable owner fixture setup: {:?}",
        setup_started.elapsed()
    );
    let budget =
        ExecutionBudget::new(Duration::from_secs(5), CancellationToken::new()).expect("budget");
    let output = transport
        .run(
            &spec,
            &json!({}),
            &[pid_file.display().to_string()],
            &WorkerLimits::default(),
            &budget,
        )
        .expect("owner cleanup");
    #[cfg(target_os = "linux")]
    assert_eq!(output.cleanup_scope, CleanupScope::OwnerReapedProcessTree);
    #[cfg(target_os = "macos")]
    assert_eq!(
        output.cleanup_scope,
        CleanupScope::OwnerTerminatedProcessGroup
    );
    let descendant: i32 = fs::read_to_string(pid_file)
        .expect("descendant pid")
        .parse()
        .expect("numeric pid");
    assert_eq!(
        unsafe { libc::kill(descendant, 0) },
        -1,
        "descendant must be reaped, not merely zombie"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[test]
#[cfg(target_os = "linux")]
fn cancellation_reaps_worker_and_session_escaping_descendant() {
    let root = tempfile::tempdir().expect("test root");
    let pid_file = root.path().join("descendant.pid");
    // These are lifecycle fixtures, not CPU stress tests. Blocking fixtures
    // avoid unrelated CPU contention inside the bounded test cgroup.
    let spec = worker(root.path(), "/usr/bin/setsid /usr/bin/dash -c 'printf \"%s\" \"$$\" > \"$1\"; exec /usr/bin/sleep 30' fixture \"$3\" & wait");
    let transport = transport(root.path());
    let token = CancellationToken::new();
    let budget = ExecutionBudget::new(Duration::from_secs(5), token.clone()).expect("budget");
    let path_for_thread = pid_file.clone();
    let cancellation = thread::spawn(move || {
        for _ in 0..400 {
            if path_for_thread.exists() {
                token.cancel();
                return Ok(());
            }
            thread::sleep(Duration::from_millis(5));
        }
        token.cancel();
        Err("worker descendant never started within the 2-second startup bound")
    });
    let result = transport.run(
        &spec,
        &json!({}),
        &[pid_file.display().to_string()],
        &WorkerLimits::default(),
        &budget,
    );
    let startup = cancellation.join().expect("cancellation thread");
    assert!(startup.is_ok(), "{startup:?}; transport result: {result:?}");
    let error = result.expect_err("cancelled");
    assert_eq!(error.code, WorkerErrorCode::Cancelled, "{error}");
    let descendant: i32 = fs::read_to_string(pid_file)
        .expect("descendant pid")
        .parse()
        .expect("numeric pid");
    assert_eq!(
        unsafe { libc::kill(descendant, 0) },
        -1,
        "escaped descendant must also be reaped"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn parent_death_still_reaps_descendants() {
    let root = tempfile::tempdir().expect("test root");
    let pid_file = root.path().join("descendant.pid");
    let spec = worker(
        root.path(),
        "/usr/bin/sleep 30 & printf '%s' \"$!\" > \"$3\"; wait",
    );
    // A separate, explicit harness owns subreaper state; the Rust test process
    // and SDK host are not changed. It adopts and waits the orphaned helper.
    let harness = r#"
import ctypes, os, signal, subprocess, sys, time
libc = ctypes.CDLL(None, use_errno=True)
assert libc.prctl(36, 1, 0, 0, 0) == 0, ctypes.get_errno()
parent = os.fork()
if parent == 0:
    child = subprocess.Popen([sys.argv[1], '__worker-owner', '--', sys.argv[2], 'unused-input', 'unused-output', sys.argv[3]], cwd=sys.argv[4])
    child.wait()
    os._exit(0)
deadline = time.monotonic() + 5
while not os.path.exists(sys.argv[3]):
    assert time.monotonic() < deadline, 'worker failed to start'
    time.sleep(0.005)
descendant = int(open(sys.argv[3]).read())
os.kill(parent, signal.SIGKILL)
os.waitpid(parent, 0)
while True:
    try:
        pid, status = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    assert time.monotonic() < deadline, 'orphan owner did not terminate/reap'
    assert pid != descendant, 'descendant leaked from owner to harness'
    if pid == 0:
        time.sleep(0.005)
try:
    os.kill(descendant, 0)
except ProcessLookupError:
    descendant_gone = True
else:
    raise AssertionError('descendant remains alive or zombie')
assert descendant_gone
"#;
    let result = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_svm-replay"))
        .arg(spec.executable)
        .arg(pid_file)
        .arg(root.path())
        .output()
        .expect("parent-death harness");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
