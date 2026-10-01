use super::*;

#[test]
fn backward_scan_matches_exhaustive_causal_path_reference() {
    for mask in 0..256_u64 {
        let mut transactions = Vec::new();
        for i in 0..4 {
            let write = if mask & (1 << (2 * i)) != 0 { "a" } else { "b" };
            let read = if mask & (1 << (2 * i + 1)) != 0 {
                "a"
            } else {
                "b"
            };
            transactions.push(semantic(i, &[write], &[write, read], "program"));
        }
        transactions.push(semantic(4, &[], &["a"], "target"));
        for failed in [None, Some(1), Some(3)] {
            let mut cases = transactions.clone();
            if let Some(index) = failed {
                cases[index].succeeded = false;
                if mask % 2 == 0 {
                    cases[index].possible_persistent_writes = Some(vec!["a".into()]);
                }
            }
            let mut expected = BTreeSet::new();
            reference_paths(&cases, 4, &BTreeSet::from(["a".into()]), &mut expected);
            assert_eq!(
                compute_backward_semantic_closure(&cases, 4, &[])
                    .expect("closure")
                    .selected_indices,
                expected.into_iter().collect::<Vec<_>>(),
                "mask={mask} failed={failed:?}"
            );
        }
    }
}

#[test]
#[ignore = "explicit development differential requires Bun and SVM_REPLAY_PROTOTYPE"]
fn original_typescript_differential_on_generated_offline_cases() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let prototype =
        std::env::var("SVM_REPLAY_PROTOTYPE").expect("explicit preserved prototype path");
    let mut cases = Vec::new();
    for i in 0..64 {
        let mut tx = raw(
            &format!("case-{i}"),
            &["payer", "a", "b", "outer"],
            i % 3,
            json!([{"programIdIndex":3,"accounts":if i%2==0 {vec![1,2]}else{vec![0,2,2]},"data":""}]),
        );
        if i % 3 == 0 {
            tx["meta"]["err"] = json!({"InstructionError":[0,"InvalidArgument"]});
        }
        if i % 4 == 0 {
            tx["meta"]["innerInstructions"] =
                json!([{"index":0,"instructions":[{"programId":"cpi","accounts":[0,1]}]}]);
        }
        cases.push(tx);
    }
    cases.extend([nonce(true, false), nonce(false, false), nonce(true, true)]);
    let script = r#"const root=process.env.SVM_REPLAY_PROTOTYPE; const {summarizeSemanticTransaction}=await import(root+'/poc/m0-correctness/transaction.ts');const {computeBackwardSemanticClosure,computeProgramDataFixedPoint}=await import(root+'/poc/m0-correctness/closure.ts');const {includeRequestedDependencies}=await import(root+'/poc/m9-requested-dependencies.ts');const inputs=await Bun.stdin.json();const summaries=inputs.map(summarizeSemanticTransaction);const additional=['request-only'];const closures=summaries.map((_,i)=>computeBackwardSemanticClosure(summaries,i,additional));const original={...summaries[0],index:1};const merged=includeRequestedDependencies(original,summaries[1]);const fixed=await computeProgramDataFixedPoint(summaries,summaries.length-4,async keys=>new Map(keys.map(key=>[key,key==='outer'?'pd-outer':null])));console.log(JSON.stringify({summaries,closures,merged,fixed}));"#;
    let mut child = Command::new("bun")
        .args(["--eval", script])
        .env("SVM_REPLAY_PROTOTYPE", prototype)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run unchanged original algorithm");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&serde_json::to_vec(&cases).expect("raw cases"))
        .expect("send generated inputs");
    let output = child.wait_with_output().expect("wait original algorithm");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = svm_replay_protocol::parse_json(&output.stdout).expect("original output JSON");
    let summaries: Vec<_> = cases
        .iter()
        .enumerate()
        .map(|(i, raw)| summary(raw, i as u64))
        .collect();
    let closures: Vec<_> = (0..summaries.len())
        .map(|i| {
            compute_backward_semantic_closure(&summaries, i as u64, &["request-only".into()])
                .expect("closure")
        })
        .collect();
    let mut original = summaries[0].clone();
    original.index = 1;
    let merged = include_requested_dependencies(&original, &summaries[1]).expect("merge");
    let fixed =
        compute_program_data_fixed_point(&summaries, (summaries.len() - 4) as u64, |keys| {
            Ok(keys
                .iter()
                .map(|key| {
                    (
                        key.clone(),
                        if key == "outer" {
                            Some("pd-outer".into())
                        } else {
                            None
                        },
                    )
                })
                .collect())
        })
        .expect("fixed point");
    assert_eq!(
        json!({"summaries":summaries,"closures":closures,"merged":merged,"fixed":fixed}),
        expected
    );
}
