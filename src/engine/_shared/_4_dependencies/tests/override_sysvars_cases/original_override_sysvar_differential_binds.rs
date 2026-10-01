use super::*;

#[test]
#[ignore = "explicit development differential requires Bun and SVM_REPLAY_PROTOTYPE"]
fn original_override_sysvar_differential_binds_full_images_and_proofs() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let features = json!([]);
    let mut cases = vec![];
    let mut actual = vec![];
    let mut keys = BANK_INITIALIZED_SYSVARS.to_vec();
    keys.push(SLOT_HASHES);
    keys.extend(LEGACY);
    for source in EXECUTORS {
        for mode in 0..6 {
            let c = OverrideSysvarContext {
                executor_source_id: source,
                ..context(&features)
            };
            let accounts = match mode {
                0 => vec![],
                1 => vec![account(SLOT_HASHES, 97)],
                2 => vec![account(SLOT_HASHES, 100)],
                3 => vec![account(LEGACY[0], 97)],
                4 => keys
                    .iter()
                    .map(|key| account(key, if *key == SLOT_HASHES { 100 } else { 97 }))
                    .collect(),
                5 => vec![account(LEGACY[0], 100)],
                _ => unreachable!(),
            };
            let mut reads = vec![];
            let result = hydrate_override_sysvar_baselines_with_accounts(
                &edits(&keys),
                &accounts,
                &c,
                |key, slot| {
                    reads.push(json!([key, slot]));
                    Ok(vec![account(key, slot)])
                },
            );
            let output = match result {
                Ok(value) => {
                    let proven =
                        proven_parent_override_sysvars(&value.accounts, &value.bindings, &c)
                            .unwrap();
                    json!({"value":value,"proven":proven,"reads":reads})
                }
                Err(e) => json!({"error":e.code,"reads":reads}),
            };
            actual.push(output);
            cases.push(json!({"accounts":accounts,"overrides":edits(&keys),"source":source}));
        }
    }
    let script = r#"const root=process.env.SVM_REPLAY_PROTOTYPE;const {hydrateOverrideSysvarBaselines,provenParentOverrideSysvars}=await import(root+'/poc/m9-override-sysvars.ts');const a=(pubkey,sourceSlot)=>({pubkey,sourceSlot,role:'sysvar',presence:'present',lamports:'9007199254740993',owner:'Sysvar1111111111111111111111111111111111111',executable:false,rentEpoch:'18446744073709551615',dataBase64:Buffer.from(`controlled:${pubkey}:${sourceSlot}`).toString('base64')});const results=[];for(const c of await Bun.stdin.json()){const reads=[];const input={...c,parentSlot:97,targetSlot:100,executorSourceId:c.source,activeExecutionFeatureIds:[],client:{accounts:async(keys,slot)=>keys.map(key=>{reads.push([key,slot]);const x=a(key,slot);return{pubkey:key,slot,value:{owner:x.owner,executable:x.executable,lamports:x.lamports,rentEpoch:x.rentEpoch,data:[x.dataBase64,'base64']}}})}};try{const value=await hydrateOverrideSysvarBaselines(input);const proven=[...provenParentOverrideSysvars({...input,accounts:value.accounts,bindings:value.bindings})].sort();results.push({value,proven,reads})}catch(e){results.push({error:e.code,reads})}}console.log(JSON.stringify(results));"#;
    let mut child = Command::new("bun")
        .args(["--eval", script])
        .env(
            "SVM_REPLAY_PROTOTYPE",
            std::env::var("SVM_REPLAY_PROTOTYPE").expect("prototype path"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = svm_replay_protocol::parse_json(&output.stdout).unwrap();
    for (i, (a, b)) in actual.iter().zip(expected.as_array().unwrap()).enumerate() {
        assert_eq!(a, b, "case {i}");
    }
    assert_eq!(actual.len(), expected.as_array().unwrap().len());
}
