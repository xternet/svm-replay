use super::*;

/// A what-if run: the original still verifies; one override and two bundle transactions.
fn what_if_context() -> (Value, Value, Value, Value) {
    let (candidate, mut fixture, block, mut output) = context();
    fixture["whatIf"] = json!({"mode":"after","transactionsBase64":["AQ==","Ag=="],
        "stateOverrides":[{"pubkey":"arb","owner":"BPFLoader2111111111111111111111111111111111",
            "executable":true,"dataBase64":STANDARD.encode(b"\x7fELF")}, {"pubkey":"gone","lamports":"0"}]});
    let absent = json!({"presence":"absent","lamports":null,"owner":null,"executable":null,
        "rentEpoch":null,"dataHash":null,"tokenAmount":null});
    let program = json!({"presence":"present","lamports":"1141440","owner":"BPFLoader2111111111111111111111111111111111",
        "executable":true,"rentEpoch":"18446744073709551615","dataHash":hash(b"\x7fELF"),"tokenAmount":null});
    let mut existing = program.clone();
    existing["executable"] = json!(false);
    let mut run = output["original"].clone();
    run.as_object_mut().unwrap().remove("endAccountStates");
    run["endAccountStates"] = json!([]);
    output["whatIf"] = json!({"mode":"after",
        "stateOverrides":[{"pubkey":"arb","before":absent,"after":program},
            {"pubkey":"gone","before":existing,"after":absent}],
        "transactions":[run.clone(), run]});
    (candidate, fixture, block, output)
}

#[test]
fn what_if_runs_verify_the_original_and_prove_the_overrides() {
    let (candidate, fixture, block, output) = what_if_context();
    let report = verify(
        &candidate,
        &fixture,
        &block,
        &output,
        MetadataPolicy::Strict,
    )
    .unwrap();
    assert_eq!(
        report.0["whatIf"],
        json!({"mode":"after","transactions":2,"stateOverridesApplied":true})
    );
}

#[test]
fn what_if_output_must_match_the_request() {
    let (candidate, fixture, block, output) = what_if_context();
    let mut cases: Vec<(&str, Value, Value)> = Vec::new();
    let mut missing = output.clone();
    missing.as_object_mut().unwrap().remove("whatIf");
    cases.push(("missing", fixture.clone(), missing));
    let mut unexpected = fixture.clone();
    unexpected.as_object_mut().unwrap().remove("whatIf");
    cases.push(("unexpected", unexpected, output.clone()));
    for (name, pointer, value) in [
        ("mode", "/whatIf/mode", json!("replace")),
        (
            "count",
            "/whatIf/transactions",
            json!([output["original"].clone()]),
        ),
        (
            "owner",
            "/whatIf/stateOverrides/0/after/owner",
            json!("other"),
        ),
        (
            "data",
            "/whatIf/stateOverrides/0/after/dataHash",
            json!(hash(b"other")),
        ),
        (
            "executable",
            "/whatIf/stateOverrides/0/after/executable",
            json!(false),
        ),
        (
            "deleted",
            "/whatIf/stateOverrides/1/after/presence",
            json!("present"),
        ),
        ("order", "/whatIf/stateOverrides/0/pubkey", json!("gone")),
        ("status", "/whatIf/transactions/1/status", json!("maybe")),
    ] {
        let mut changed = output.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        cases.push((name, fixture.clone(), changed));
    }
    for (name, fixture, output) in cases {
        assert!(
            verify(
                &candidate,
                &fixture,
                &block,
                &output,
                MetadataPolicy::Strict
            )
            .is_err(),
            "{name}"
        );
    }
}
