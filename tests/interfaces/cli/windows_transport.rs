#![cfg(windows)]
//! Synthetic process tests only; historical parity is tested by the corpus.
use std::{fs, process::Command, thread, time::Duration};
use svm_replay_engine::shared::runtime::{
    file_sha256, CancellationToken, ExecutionBudget, WorkerErrorCode, WorkerLimits, WorkerSpec,
    WorkerTransport,
};

fn fixture(root: &std::path::Path) -> WorkerSpec {
    let source = root.join("worker.rs");
    fs::write(
        &source,
        r#"
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(3).map(String::as_str) == Some("debug") {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        eprintln!("{}", listener.local_addr().unwrap().port());
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut byte = [0];
        let mut request = Vec::new();
        loop {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            if byte[0] == b'#' { break; }
        }
        let mut checksum = [0; 2];
        stream.read_exact(&mut checksum).unwrap();
        assert_eq!(request, b"$qRcmd,6d65746164617461#");
        stream.write_all(b"+").unwrap();
        for response in ["O7b7d", "OK"] {
            let sum = response.bytes().fold(0u8, |sum, b| sum.wrapping_add(b));
            stream.write_all(format!("${response}#{sum:02x}").as_bytes()).unwrap();
            stream.read_exact(&mut byte).unwrap();
            assert_eq!(byte[0], b'+');
        }
        std::fs::write(&args[2], b"{}").unwrap();
        return;
    }
    if args.get(3).map(String::as_str) == Some("socket") {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let client = std::net::TcpStream::connect(address).unwrap();
        let (_server, peer) = listener.accept().unwrap();
        assert_eq!(peer, client.local_addr().unwrap());
        std::fs::write(&args[2], format!("{{\"port\":{}}}", address.port())).unwrap();
        return;
    }
    if args.get(1).map(String::as_str) == Some("child") {
        std::thread::sleep(std::time::Duration::from_secs(30));
        return;
    }
    if args.get(3).map(String::as_str) == Some("tree") {
        let child = std::process::Command::new(std::env::current_exe().unwrap()).arg("child").spawn().unwrap();
        std::fs::write(&args[2], format!("{{\"child\":{}}}", child.id())).unwrap();
        return;
    }
    if args.get(3).map(String::as_str) == Some("sleep") {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
    std::fs::write(&args[2], b"{}").unwrap();
}
"#,
    )
    .expect("synthetic fixture source");
    let executable = root.join("worker.exe");
    let build = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("compile native lifecycle fixture");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    WorkerSpec {
        sha256: file_sha256(&executable).expect("worker pin"),
        executable,
    }
}

#[test]
fn native_debugger_streams_diagnostics_and_exchanges_metadata_before_exit() {
    let root = tempfile::tempdir().expect("test root");
    let worker = fixture(root.path());
    let budget = ExecutionBudget::new(Duration::from_secs(15), CancellationToken::new()).unwrap();
    let (sender, notices) = std::sync::mpsc::sync_channel(16);
    thread::scope(|scope| {
        let handle = scope.spawn(|| {
            WorkerTransport::new(root.path()).run_with_diagnostics(
                &worker,
                &serde_json::json!({}),
                &["debug".into()],
                &[],
                &WorkerLimits::default(),
                &budget,
                &sender,
            )
        });
        let mut line = Vec::new();
        while !line.contains(&b'\n') {
            line.extend(
                notices
                    .recv_timeout(Duration::from_secs(5))
                    .expect("live notice"),
            );
        }
        let port = std::str::from_utf8(&line).unwrap().trim().parse().unwrap();
        let mut client =
            svm_replay_engine::shared::debug::DebugClient::connect(port, 32768, &budget)
                .expect("debugger attachment");
        assert_eq!(client.metadata().expect("live RSP metadata"), "{}");
        assert_eq!(
            handle.join().unwrap().expect("worker completion").value,
            serde_json::json!({})
        );
    });
}

#[test]
fn native_worker_can_open_debugger_loopback_socket() {
    let root = tempfile::tempdir().expect("test root");
    let worker = fixture(root.path());
    let budget = ExecutionBudget::new(Duration::from_secs(10), CancellationToken::new()).unwrap();
    let output = WorkerTransport::new(root.path())
        .run(
            &worker,
            &serde_json::json!({}),
            &["socket".into()],
            &WorkerLimits::default(),
            &budget,
        )
        .expect("native debugger loopback networking");
    assert!(output.value["port"].as_u64().is_some_and(|port| port > 0));
}

#[test]
fn native_worker_executes_and_cancellation_waits_for_termination() {
    let root = tempfile::tempdir().expect("test root");
    let worker = fixture(root.path());
    let transport = WorkerTransport::new(root.path());
    let budget = ExecutionBudget::new(Duration::from_secs(10), CancellationToken::new()).unwrap();
    let output = transport
        .run(
            &worker,
            &serde_json::json!({}),
            &[],
            &WorkerLimits::default(),
            &budget,
        )
        .expect("native execution");
    assert_eq!(output.value, serde_json::json!({}));
    let tree = transport
        .run(
            &worker,
            &serde_json::json!({}),
            &["tree".into()],
            &WorkerLimits::default(),
            &budget,
        )
        .expect("descendant cleanup");
    let pid = tree.value["child"].as_u64().unwrap() as u32;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(ERROR_INVALID_PARAMETER as i32)
        );
    } else {
        let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
        assert_eq!(
            unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) },
            WAIT_OBJECT_0,
            "descendant must have terminated before transport returns"
        );
    }
    let cancel = CancellationToken::new();
    let budget = ExecutionBudget::new(Duration::from_secs(10), cancel.clone()).unwrap();
    let trigger = thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        cancel.cancel();
    });
    let error = transport
        .run(
            &worker,
            &serde_json::json!({}),
            &["sleep".into()],
            &WorkerLimits::default(),
            &budget,
        )
        .expect_err("cancelled execution");
    trigger.join().unwrap();
    assert_eq!(error.code, WorkerErrorCode::Cancelled, "{error}");
    assert!(error.pid.is_some(), "worker must have actually started");
}
