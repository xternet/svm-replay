use super::*;

const LOADER: &str = "BPFLoader1111111111111111111111111111111111";

#[test]
fn implicit_loader_uses_historical_bytes_not_runtime_placeholder() {
    let program = json!({"pubkey":"program", "presence":"present", "executable":true,
        "owner":LOADER});
    let loader = json!({"pubkey":LOADER,"presence":"present","executable":true,
        "owner":"NativeLoader1111111111111111111111111111111",
        "dataBase64":STANDARD.encode(b"solana_bpf_loader_program")});
    let mut records = vec![program.clone(), program];
    let mut calls = 0;
    include_loaders(&mut records, |key| {
        assert_eq!(key, LOADER);
        calls += 1;
        Ok(loader.clone())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(records.last(), Some(&loader));
    // The historical legacy loader is 25 bytes; LiteSVM's default is 36.
    assert_eq!(
        STANDARD
            .decode(records.last().unwrap()["dataBase64"].as_str().unwrap())
            .unwrap()
            .len(),
        b"solana_bpf_loader_program".len()
    );
    include_loaders(&mut records, |_| panic!("already supplied")).unwrap();
}

#[test]
fn missing_implicit_loader_is_not_replaced_with_a_default() {
    let mut records = vec![json!({"pubkey":"program","presence":"present",
        "executable":true,"owner":LOADER})];
    let error = include_loaders(&mut records, |key| {
        Ok(json!({"pubkey":key,"presence":"absent"}))
    })
    .unwrap_err();
    assert_eq!(error.code, "UNSUPPORTED_HISTORICAL_ACCOUNT");
}

#[test]
fn ordinary_accounts_and_native_builtins_need_no_implicit_loader() {
    let mut records = vec![
        json!({"pubkey":"token","presence":"present",
        "executable":false,"owner":LOADER}),
        json!({"pubkey":"builtin","presence":"present",
        "executable":true,"owner":"NativeLoader1111111111111111111111111111111"}),
    ];
    include_loaders(&mut records, |_| panic!("not an implicit BPF loader")).unwrap();
    assert_eq!(records.len(), 2);
}
