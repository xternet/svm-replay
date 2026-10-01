#![cfg(target_os = "linux")]
#[path = "../../../src/engine/_shared/_6_fees/tests/support.rs"]
mod fee_ledger;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
use svm_replay_engine::{
    shared::{
        cache::identity::derive,
        runtime::{file_sha256, CancellationToken, ProcessOwner},
        trace::{CaptureRequest, ProducerBounds},
    },
    simulate_prepared, CacheMode, Config, TraceOptions,
};
use svm_replay_protocol::{Digest, PreparedRequest};

// Explicit synthetic fee-ledger protocol fixture, not an SVM/historical parity oracle.
const WORKER: &str = r#"#!/usr/bin/python3
import base64, copy, hashlib, json, pathlib, platform, sys
fixture=json.loads(pathlib.Path(sys.argv[1]).read_bytes())
mode=sys.argv[3]
def encoded(v): return json.dumps(v,sort_keys=True,separators=(',',':')).encode()
def digest(v): return hashlib.sha256(v).hexdigest()
account=fixture['accounts'][0]
fee=int(fixture['runtime']['bankContext']['lamportsPerSignature'])
def state(balance):
    return dict(presence='present',lamports=str(balance),owner=account['owner'],executable=account['executable'],rentEpoch=account['rentEpoch'],dataHash=digest(base64.b64decode(account['dataBase64'])),tokenAmount=None)
def execute(balance,end=False):
    return dict(status='ok',normalizedError=None,logs=[],computeUnits=0,fee=str(fee),returnData=None,accountTransitions=[dict(pubkey=account['pubkey'],before=state(balance),after=state(balance-fee))],endAccountStates=[dict(pubkey=account['pubkey'],state=state(balance-fee))] if end else [])
canonical=copy.deepcopy(fixture)
canonical.pop('accountOverride'); canonical.pop('requestedAccountOverrides',None)
canonical['target'].pop('replacementTransactionBase64')
metrics=dict(prefixTransactionsExecuted=0,prefixSimulationCalls=0,prefixCommitCalls=0,prefixTransactionsReused=0)
if mode=='--checkpoint-run':
    raw=pathlib.Path(sys.argv[4]).read_bytes(); assert digest(raw)==sys.argv[5]
    checkpoint=json.loads(raw); assert checkpoint['fixtureSha256']==digest(encoded(canonical))
    balance=checkpoint['image']['balance']; prefix=checkpoint['prefix']
    metrics['prefixTransactionsReused']=len(prefix)
else:
    balance=int(account['lamports']); prefix=[]
    for index,signature in zip(fixture['target']['prefixIndices'],fixture['target']['prefixSignatures']):
        prefix.append(dict(index=index,signature=signature,**execute(balance))); balance-=fee
    for name in ['prefixTransactionsExecuted','prefixSimulationCalls','prefixCommitCalls']: metrics[name]=len(prefix)
original=execute(balance,True)
result=dict(schema='svm-simulate-m6-executor-output/v1',caseId=fixture['caseId'],executorSourceId=fixture['runtime']['binding']['executor']['id'],runtimeBindingHash=fixture['runtime']['binding']['bindingHash'],targetIndex=fixture['target']['index'],targetSignature=fixture['target']['signature'],prefix=prefix,omittedFeeEffects=[],original=original,replacement=None,accountOverride=None)
if mode=='--checkpoint-create':
    checkpoint=dict(schema='svm-m10-boundary/v1',fixtureSha256=digest(encoded(canonical)),workerSha256=digest(pathlib.Path(__file__).read_bytes()),image=dict(balance=balance),prefix=prefix,omittedFeeEffects=[])
    raw=encoded(checkpoint)
    with pathlib.Path(sys.argv[4]).open('xb') as out: out.write(raw)
    response=dict(schema='svm-m10-checkpoint-output/v1',status='CREATED',checkpointSha256=digest(raw),checkpointBytes=len(raw),metrics=metrics,reads=[])
elif mode=='--checkpoint-run':
    response=dict(schema='svm-m10-checkpoint-output/v1',status='EXECUTED',checkpointSha256=digest(raw),output=result,metrics=metrics,reads=[])
else:
    response=dict(schema='svm-sysvar-discovery/v1',status='COMPLETE',reads=[],output=result)
    if mode=='--capture':
        def accounts(amount): return [dict(pubkey=account['pubkey'],state=dict(lamports=str(amount),owner=account['owner'],executable=account['executable'],rent_epoch=account['rentEpoch'],data=list(base64.b64decode(account['dataBase64']))))]
        events=[dict(kind='transaction_begin',accounts=accounts(balance)),dict(kind='transaction_commit',outcome=dict(status='success'),accounts=accounts(balance-fee))]
        journal=dict(signatures=[fixture['target']['signature']],scope='synthetic fee-only target',before_scope='tracked account',before_keys=[account['pubkey']],terminal_scope='committed tracked account',terminal_keys=[account['pubkey']],journal=dict(disposition='COMPLETE',reason=None,event_count=len(events),payload_json=json.dumps(events)))
        response=dict(schema='svm-historical-capture-worker-experimental/v1',status='COMPLETE',execution_mode='jit' if platform.machine()=='x86_64' else 'interpreter',output=response,captures=[],journals=[journal])
pathlib.Path(sys.argv[2]).write_bytes(encoded(response))
"#;

fn setup(
    root: &Path,
) -> (
    PreparedRequest,
    Config,
    svm_replay_protocol::worker::WorkerDescriptor,
) {
    let (mut request, reference) = fee_ledger::fixture(root, WORKER);
    let mut clock = [0u8; 40];
    clock[..8].copy_from_slice(&20u64.to_le_bytes());
    request.fixture["clock"] = json!({"pubkey":"SysvarC1ock11111111111111111111111111111111","sourceSlot":20,"role":"sysvar","presence":"present","owner":"Sysvar1111111111111111111111111111111111111","executable":false,"lamports":"1","rentEpoch":"18446744073709551615","dataBase64":STANDARD.encode(clock)});
    let context = Digest::of(b"synthetic coordinator test context");
    request.fixture["runtime"]["lastRestartSlot"] = json!({"bindingHash":context});
    request.fixture["runtime"]["bankContext"] = json!({"targetSlot":20,"parentSlot":19,"lamportsPerSignature":5,"executionBlockhash":"synthetic","sourceKind":"synthetic","blockSourceHash":request.block_sha256,"blockLineageHash":context,"feeWitnessCount":0,"feeWitnessIndicesHash":Digest::of(b"[]"),"feeWitnessHash":Digest::of(b"[]"),"requiredRuntimeSysvars":[]});
    let capture_file = root.join("capture");
    fs::write(
        &capture_file,
        format!("{WORKER}\n# distinct capture role\n"),
    )
    .unwrap();
    fs::set_permissions(&capture_file, fs::Permissions::from_mode(0o700)).unwrap();
    let mut capture = reference.descriptor.clone();
    capture.file = "capture".into();
    capture.sha256 = Digest::new(file_sha256(&capture_file).unwrap()).unwrap();
    capture.capabilities.extend([
        "runtime-full-capture-cli/v1".into(),
        "runtime-call-journal/v1".into(),
        "runtime-interpreter-capture/v1".into(),
    ]);
    let catalog = json!({"schema":"svm-replay-workers/v1","platform":{"os":"linux","arch":std::env::consts::ARCH,"glibcMin":"2.17"},"workers":[reference.descriptor],"captureWorkers":[{"worker":capture,"referenceSha256":reference.descriptor.sha256,"gateSha256":Digest::of(b"explicit synthetic gate")} ]});
    let catalog_bytes = serde_json::to_vec(&catalog).unwrap();
    let catalog_path = root.join("catalog.json");
    fs::write(&catalog_path, &catalog_bytes).unwrap();
    let owner = root.join("owner");
    fs::copy(env!("CARGO_BIN_EXE_svm-replay"), &owner).unwrap();
    let config = Config {
        catalog_path,
        catalog_sha256: Digest::of(catalog_bytes),
        data_dir: root.join("data"),
        owner: ProcessOwner {
            sha256: file_sha256(&owner).unwrap(),
            executable: owner,
        },
        cache: CacheMode::All,
        trace: None,
    };
    (request, config, reference.descriptor)
}

#[test]
fn public_job_uses_actual_host_not_distinct_owner_for_cache_and_trace_identity() {
    let root = tempfile::tempdir().unwrap();
    let (request, mut config, worker) = setup(root.path());
    let host = Digest::new(file_sha256(&std::env::current_exe().unwrap()).unwrap()).unwrap();
    let helper = Digest::new(&config.owner.sha256).unwrap();
    assert_ne!(
        host, helper,
        "this regression requires a distinct actual host/helper"
    );
    let cold = simulate_prepared(request.clone(), &config, CancellationToken::new()).unwrap();
    assert_eq!(cold["outcome"], "COMPLETED", "{cold}");
    assert!(config.data_dir.join("cache/index.sqlite").is_file());
    assert!(config.data_dir.join("cache/blobs").is_dir());
    assert!(!config.data_dir.join("index.sqlite").exists());
    assert!(!config.data_dir.join("jobs").exists());
    assert!(
        Path::new(cold["receiptPath"].as_str().unwrap()).starts_with(config.data_dir.join("runs"))
    );
    assert_eq!(cold["implementationSha256"], json!(host));
    assert_eq!(cold["processOwnerSha256"], json!(helper));
    let path = Path::new(cold["receiptPath"].as_str().unwrap())
        .parent()
        .unwrap();
    let prepared: PreparedRequest =
        serde_json::from_slice(&fs::read(path.join("prepared-request.json")).unwrap()).unwrap();
    let expected = derive(&prepared, &worker, &config.catalog_sha256, &host).unwrap();
    let wrong = derive(&prepared, &worker, &config.catalog_sha256, &helper).unwrap();
    assert_eq!(cold["cache"]["preparedKey"], json!(expected.prepared_key));
    assert_eq!(cold["cache"]["resultKey"], json!(expected.result_key));
    assert_ne!(expected.prepared_key, wrong.prepared_key);
    assert_ne!(expected.result_key, wrong.result_key);
    let hit = simulate_prepared(request.clone(), &config, CancellationToken::new()).unwrap();
    assert_eq!(hit["outcome"], "COMPLETED", "{hit}");
    assert_eq!(hit["cache"]["newExecution"], false);
    let mode = if cfg!(target_arch = "x86_64") {
        "jit"
    } else {
        "interpreter"
    };
    config.trace=Some(TraceOptions {require_complete:true,capture:CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":mode,"level":"calls","sbpfObservations":"none","filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":1048576,"maxEvents":100,"timeoutMs":10000}})).unwrap()).unwrap(),bounds:ProducerBounds {register_rows:0,memory_rows:None,max_invocations:64}});
    let traced = simulate_prepared(request.clone(), &config, CancellationToken::new()).unwrap();
    assert_eq!(traced["outcome"], "COMPLETED", "{traced}");
    assert_eq!(
        traced["trace"]["execution"]["implementationSha256"],
        json!(host)
    );
    assert_eq!(traced["processOwnerSha256"], json!(helper));
    let started: Value = serde_json::from_slice(
        &fs::read(
            Path::new(traced["receiptPath"].as_str().unwrap())
                .parent()
                .unwrap()
                .join("started.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(started["implementationSha256"], json!(host));
    assert_eq!(started["processOwnerSha256"], json!(helper));
    config.trace.as_mut().unwrap().capture.limits.max_events = 1;
    let partial = simulate_prepared(request.clone(), &config, CancellationToken::new()).unwrap();
    assert_eq!(partial["outcome"], "ERROR", "{partial}");
    assert_eq!(partial["error"]["code"], "COLLECTION_INCOMPLETE");
    assert_eq!(
        partial["trace"]["exports"][0]["artifact"]["status"],
        "TRUNCATED"
    );
    let persisted: Value =
        serde_json::from_slice(&fs::read(partial["receiptPath"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(persisted["outcome"], "ERROR");
    assert_eq!(persisted["error"]["code"], "COLLECTION_INCOMPLETE");
    let trace = config.trace.as_mut().unwrap();
    trace.require_complete = false;
    trace.capture.limits.max_bytes = 2;
    let tiny = simulate_prepared(request, &config, CancellationToken::new()).unwrap();
    assert_eq!(tiny["outcome"], "COMPLETED", "{tiny}");
    assert_eq!(
        tiny["trace"]["exports"][0]["artifact"]["status"],
        "TRUNCATED"
    );
}
