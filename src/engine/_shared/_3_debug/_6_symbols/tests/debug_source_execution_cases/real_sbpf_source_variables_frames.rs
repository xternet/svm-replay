use super::*;

#[test]
#[ignore = "requires preserved pinned source-control probe and authorized artifact directory"]
fn real_sbpf_source_variables_frames_next_finish_and_optimized_unavailable() {
    let env = |name: &str| std::env::var(name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let prototype = std::path::PathBuf::from(env("SVM_REPLAY_TEST_SOURCE_FRAMES"));
    let root = tempfile::Builder::new()
        .prefix("source-control-")
        .tempdir_in(env("SVM_REPLAY_TEST_SOURCE_WORK_ROOT"))
        .unwrap()
        .keep();
    eprintln!("controlled SBPF source artifacts: {}", root.display());
    let receipt = parse_json(
        &read_bounded_file(&prototype.join("results/final-5/receipt.json"), 1024 * 1024).unwrap(),
    )
    .unwrap();
    let probe_source = prototype.join("target/debug/m14-source-frame-probe");
    let probe_sha = receipt["hashes"][probe_source.to_str().unwrap()]
        .as_str()
        .unwrap();
    assert_eq!(file_sha256(&probe_source).unwrap(), probe_sha);
    let probe = root.join("probe");
    fs::copy(&probe_source, &probe).unwrap();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o500)).unwrap();
    let adapter = root.join("probe-adapter");
    fs::write(&adapter, ADAPTER).unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o500)).unwrap();
    let worker = WorkerSpec {
        sha256: file_sha256(&adapter).unwrap(),
        executable: adapter,
    };
    let transport = WorkerTransport::new(&root).with_owner(ProcessOwner {
        executable: env("SVM_REPLAY_TEST_TRACE_OWNER").into(),
        sha256: env("SVM_REPLAY_TEST_TRACE_OWNER_SHA256"),
    });
    let mut proof = Vec::new();
    for (name, input, pin) in [
        (
            "program.so",
            17u8,
            receipt["admission"]["elfSha256"].as_str().unwrap(),
        ),
        (
            "optimized.so",
            41u8,
            receipt["optimized"]["setup"]["elfSha256"].as_str().unwrap(),
        ),
    ] {
        let elf = root.join(name);
        fs::copy(prototype.join("results/final-5").join(name), &elf).unwrap();
        fs::set_permissions(&elf, fs::Permissions::from_mode(0o400)).unwrap();
        assert_eq!(file_sha256(&elf).unwrap(), pin);
        let symbols = ExactSymbols::admit(&elf, &Digest::new(pin).unwrap()).unwrap();
        assert!(symbols.available());
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut fixture = json!({"probe":probe,"probeSha256":probe_sha,"elf":elf,"elfSha256":pin,"input":input,"port":port,"mode":"jit"});
        let budget =
            ExecutionBudget::new(Duration::from_secs(60), CancellationToken::new()).unwrap();
        let jit = transport
            .run(&worker, &fixture, &[], &WorkerLimits::default(), &budget)
            .unwrap();
        fixture["mode"] = json!("interpreted");
        let interpreted = transport
            .run(&worker, &fixture, &[], &WorkerLimits::default(), &budget)
            .unwrap();
        assert_eq!(jit.value["output"], interpreted.value["output"]);
        fixture["mode"] = json!("debug");
        let (sender, notices) = mpsc::sync_channel(64);
        let mut observations = Vec::new();
        let debug = thread::scope(|scope| {
            let running = scope.spawn(|| {
                transport.run_with_diagnostics(
                    &worker,
                    &fixture,
                    &[],
                    &[],
                    &WorkerLimits::default(),
                    &budget,
                    &sender,
                )
            });
            let mut diagnostics = Vec::new();
            loop {
                budget.check().unwrap();
                match notices.recv_timeout(Duration::from_millis(100)) {
                    Ok(bytes) => {
                        diagnostics.extend(bytes);
                        if String::from_utf8_lossy(&diagnostics).contains(&format!(
                            "Waiting for bounded debugger on 127.0.0.1:{port}\n"
                        )) {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => assert!(
                        !running.is_finished(),
                        "probe exited before listener; diagnostics={}",
                        String::from_utf8_lossy(&diagnostics)
                    ),
                    Err(error) => panic!("listener channel: {error}"),
                }
            }
            let mut client = DebugClient::connect(port, 32768, &budget).unwrap();
            symbols.bind_runtime(&mut client).unwrap();
            if name == "program.so" {
                let foreign = ExactSymbols::admit(
                    &prototype.join("results/final-5/optimized.so"),
                    &Digest::new(receipt["optimized"]["setup"]["elfSha256"].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    foreign.bind_runtime(&mut client).unwrap_err().code,
                    "SYMBOL_IDENTITY"
                );
                assert_eq!(
                    client.state(),
                    svm_replay_engine::shared::debug::DebugState::Stopped
                );
            }
            let source = if name == "program.so" {
                "program.c"
            } else {
                "optimized.c"
            };
            let line = if name == "program.so" { 17 } else { 6 };
            let points = symbols.line_addresses(source, line).unwrap();
            for pc in &points {
                client.breakpoint(*pc, true).unwrap();
            }
            client.resume().unwrap();
            assert_eq!(client.wait_stop().unwrap(), Stop::Stopped { signal: 5 });
            for pc in &points {
                client.breakpoint(*pc, false).unwrap();
            }
            let before = symbols.variable(&mut client, "before").unwrap();
            assert_eq!(before["value"], input.to_string(), "{before}");
            observations.push(before);
            if name == "program.so" {
                let points = symbols.line_addresses("program.c", 4).unwrap();
                for pc in &points {
                    client.breakpoint(*pc, true).unwrap();
                }
                client.resume().unwrap();
                assert_eq!(client.wait_stop().unwrap(), Stop::Stopped { signal: 5 });
                for pc in &points {
                    client.breakpoint(*pc, false).unwrap();
                }
                let prologue = symbols.variable(&mut client, "value").unwrap();
                assert_eq!(prologue["available"], false, "{prologue}");
                observations.push(prologue);
                nav(
                    &symbols,
                    &mut client,
                    SourceNavigationKind::Next,
                    &mut observations,
                );
                let value = symbols.variable(&mut client, "value").unwrap();
                assert_eq!(value["value"], input.to_string(), "{value}");
                observations.push(value);
                nav(
                    &symbols,
                    &mut client,
                    SourceNavigationKind::Next,
                    &mut observations,
                );
                let result = symbols.variable(&mut client, "result").unwrap();
                assert_eq!(
                    result["value"],
                    (u64::from(input) + 1).to_string(),
                    "{result}"
                );
                observations.push(result);
                let frames = symbols.frames(&mut client).unwrap();
                assert_eq!(frames["depth"], 2);
                assert_eq!(
                    frames["frames"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| f["function"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    vec!["increment", "outer", "entrypoint"]
                );
                observations.push(frames);
                nav(
                    &symbols,
                    &mut client,
                    SourceNavigationKind::Finish,
                    &mut observations,
                );
                nav(
                    &symbols,
                    &mut client,
                    SourceNavigationKind::Next,
                    &mut observations,
                );
                let after = symbols.variable(&mut client, "after").unwrap();
                assert_eq!(
                    after["value"],
                    (u64::from(input) + 1).to_string(),
                    "{after}"
                );
                observations.push(after);
                nav(
                    &symbols,
                    &mut client,
                    SourceNavigationKind::Finish,
                    &mut observations,
                );
                nav(
                    &symbols,
                    &mut client,
                    SourceNavigationKind::Next,
                    &mut observations,
                );
                let after = symbols.variable(&mut client, "after").unwrap();
                assert_eq!(
                    after["value"],
                    ((u64::from(input) + 1) * 2).to_string(),
                    "{after}"
                );
                observations.push(after);
                let absent = symbols.variable(&mut client, "does_not_exist").unwrap();
                assert_eq!(absent["available"], false);
                observations.push(absent);
            } else {
                let unavailable = symbols.variable(&mut client, "unavailable").unwrap();
                assert_eq!(unavailable["available"], false);
                assert!(unavailable["reason"]
                    .as_str()
                    .unwrap()
                    .contains("optimized"));
                observations.push(unavailable);
            }
            client.resume().unwrap();
            assert!(matches!(client.wait_stop().unwrap(), Stop::VmExited { .. }));
            client.close().unwrap();
            running.join().unwrap().unwrap()
        });
        assert_eq!(jit.value["output"], debug.value["output"]);
        assert_eq!(file_sha256(&elf).unwrap(), pin);
        proof.push(json!({"elf":name,"elfSha256":pin,"admission":symbols.admission(),"jit":jit.value,"interpreted":interpreted.value,"debug":debug.value,"process":{"pid":debug.pid,"cleanupScope":format!("{:?}",debug.cleanup_scope),"elapsedMs":debug.elapsed.as_millis(),"stderr":String::from_utf8(debug.stderr).unwrap()},"observations":observations}));
    }
    assert_eq!(file_sha256(&probe).unwrap(), probe_sha);
    fs::write(root.join("receipt.json"),serde_json::to_vec(&json!({"status":"PASS","scope":"controlled SBPFv2 source fixture; not historical source availability","probeSha256":probe_sha,"proof":proof})).unwrap()).unwrap();
}
