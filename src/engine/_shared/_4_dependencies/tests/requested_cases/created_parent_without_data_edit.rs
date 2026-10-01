use super::*;

#[test]
fn created_parent_without_data_edit_or_for_existing_table_keeps_original_role() {
    let raws = vec![mutation(0, false, false)];
    let selected = summaries(&raws);
    let roles = roles();
    let lamports = validate_requested_overrides(&json!([{"pubkey":key(3),"lamports":"3000000"}]))
        .expect("edit");
    for (records, edits, keys) in [
        (vec![absent()], vec![], vec![key(3)]),
        (vec![absent()], lamports, vec![key(3)]),
        (vec![absent()], edit(&table(4, 99, 0)), vec![]),
        (
            vec![account(ALT, &table(2, 99, 0))],
            edit(&table(4, 99, 0)),
            vec![key(3)],
        ),
    ] {
        let result = prepare_created_alt_parents(CreatedAltInput {
            records: &records,
            roles: &roles,
            parent_slot: 99,
            target_slot: 100,
            target_index: 1,
            selected: &selected,
            raw_transactions: &raws,
            requested_table_keys: &keys,
            overrides: &edits,
        })
        .expect("unchanged role");
        assert!(result.created_tables.is_empty());
        assert!(result.parent_roles.address_tables.contains(&key(3)));
    }
}

#[test]
fn history_adapter_keeps_network_slot_and_read_budget_scope() {
    use svm_replay_engine::shared::{
        dependencies::requested::resolve_requested_dependencies,
        history::History,
        runtime::{CancellationToken, ExecutionBudget},
        sources::{CompositeSource, HistoricalSource, SourceError},
    };
    use svm_replay_protocol::Digest;
    struct Source(Value);
    impl HistoricalSource for Source {
        fn identity(&self) -> Value {
            json!({"id":"exact-alt-test","version":"1","genesisHash":key(1),"kind":"captured-history","coverage":{"firstSlot":99,"lastSlot":100,"completeness":"partial"},"capabilities":["account"]})
        }
        fn inspect(&self, query: &Value) -> Result<Option<Value>, SourceError> {
            assert_eq!(
                query,
                &json!({"kind":"account","genesisHash":key(1),"slot":99,"pubkey":key(3),"phase":"end-slot"})
            );
            Ok(Some(
                json!({"query":query,"value":self.0,"evidenceHashes":[Digest::of(serde_json::to_vec(&self.0).expect("record bytes"))]}),
            ))
        }
    }
    let mut source = CompositeSource::new(vec![Box::new(Source(account(ALT, &table(4, 99, 0))))])
        .expect("source");
    let budget = ExecutionBudget::new(std::time::Duration::from_secs(10), CancellationToken::new())
        .expect("budget");
    let mut history = History::new(&mut source, key(1), 100, &budget, 1).expect("history");
    let resolved = resolve_requested_dependencies(
        &wire(true, 0),
        &original(false),
        &[],
        0,
        99,
        &mut history,
        Some(&boundary(vec![])),
        &[],
    )
    .expect("exact adapter");
    assert!(resolved.summary.declared_accounts.contains(&key(4)));
    assert_eq!(
        resolve_requested_dependencies(
            &wire(true, 0),
            &original(false),
            &[],
            0,
            99,
            &mut history,
            Some(&boundary(vec![])),
            &[]
        )
        .expect_err("cumulative read budget")
        .code,
        "UNSUPPORTED_RESOURCE_LIMIT"
    );
    let mut history =
        History::new(&mut source, key(9), 100, &budget, 1).expect("valid different genesis");
    assert_eq!(
        resolve_requested_dependencies(
            &wire(true, 0),
            &original(false),
            &[],
            0,
            99,
            &mut history,
            Some(&boundary(vec![])),
            &[]
        )
        .expect_err("network differs")
        .code,
        "SOURCE_CONTEXT_MISMATCH"
    );
}

#[test]
#[ignore = "explicit development differential requires Bun and SVM_REPLAY_PROTOTYPE"]
fn original_requested_resolver_differential_for_lifecycle_matrix() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let prototype = std::env::var("SVM_REPLAY_PROTOTYPE").expect("explicit prototype path");
    let mut cases = Vec::new();
    let mut actual = Vec::new();
    for inner in [false, true] {
        for tag in [0, 1, 2, 3, 4, 99] {
            for failed in [false, true] {
                for parent in [
                    account(ALT, &table(2, 99, 0)),
                    absent(),
                    account(&key(0), ""),
                ] {
                    let raws = vec![mutation(tag, failed, inner)];
                    let edits = edit(&table(4, 99, 0));
                    let canonical = original(true);
                    let outcome = match resolve(&canonical, &raws, parent.clone(), &edits, 0) {
                        Ok(value) => serde_json::to_value(value).expect("resolved result"),
                        Err(error) => {
                            json!({"error":if error.code=="UNSUPPORTED_ALT_LIFECYCLE" {error.code.as_str()}else{error.message.split(':').next().expect("error message segment")}})
                        }
                    };
                    actual.push(outcome);
                    cases.push(json!({"wire":wire(true,0),"original":canonical,"raws":raws,"parent":parent,"overrides":edits}));
                }
            }
        }
    }
    let script = r#"const root=process.env.SVM_REPLAY_PROTOTYPE;const {resolveRequestedDependencies}=await import(root+'/poc/m9-requested-dependencies.ts');const {summarizeSemanticTransaction}=await import(root+'/poc/m0-correctness/transaction.ts');const cases=await Bun.stdin.json();const results=[];for(const c of cases){const a=c.parent;const parent={pubkey:a.pubkey,slot:a.sourceSlot,value:a.presence==='absent'?null:{owner:a.owner,executable:a.executable,lamports:a.lamports,rentEpoch:a.rentEpoch,data:[a.dataBase64,'base64']}};try{results.push(await resolveRequestedDependencies(c.wire,c.original,c.raws.map(summarizeSemanticTransaction),c.raws.length,99,{accounts:async()=>[parent]},{targetSlot:100,rawTransactions:c.raws},c.overrides));}catch(e){results.push({error:e.code==='UNSUPPORTED_ALT_LIFECYCLE'?e.code:e.message.split(':')[0]});}}console.log(JSON.stringify(results));"#;
    let mut child = Command::new("bun")
        .args(["--eval", script])
        .env("SVM_REPLAY_PROTOTYPE", prototype)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("original resolver");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&serde_json::to_vec(&cases).expect("generated lifecycle matrix"))
        .expect("send original inputs");
    let output = child.wait_with_output().expect("wait original");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = svm_replay_protocol::parse_json(&output.stdout).expect("original result");
    assert_eq!(Value::Array(actual), expected);
}
