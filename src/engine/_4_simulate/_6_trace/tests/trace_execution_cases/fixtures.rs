use super::*;

// A fee-only protocol fixture computes its state, not historical SVM evidence.
pub(super) const WORKER: &str = r#"#!/usr/bin/python3
import base64, hashlib, json, pathlib, platform, sys
fixture=json.loads(pathlib.Path(sys.argv[1]).read_bytes())
mode=sys.argv[3] if len(sys.argv)>3 else 'baseline'
root=pathlib.Path(__file__).parent
with (root/'calls').open('a') as log:
    log.write(mode+'\n')
account=fixture['accounts'][0]
fee=int(fixture['runtime']['bankContext']['lamportsPerSignature'])
def state(balance):
    return dict(presence='present',lamports=str(balance),owner=account['owner'],executable=account['executable'],rentEpoch=account['rentEpoch'],dataHash=hashlib.sha256(base64.b64decode(account['dataBase64'])).hexdigest(),tokenAmount=None)
def run(balance,end=False):
    return dict(status='ok',normalizedError=None,logs=[],computeUnits=0,fee=str(fee),returnData=None,accountTransitions=[dict(pubkey=account['pubkey'],before=state(balance),after=state(balance-fee))],endAccountStates=[dict(pubkey=account['pubkey'],state=state(balance-fee))] if end else [])
balance=int(account['lamports']);prefix=[]
for index,signature in zip(fixture['target']['prefixIndices'],fixture['target']['prefixSignatures']):
    prefix.append(dict(index=index,signature=signature,**run(balance)))
    balance-=fee
original=run(balance,True)
if (root/'corrupt-off').exists() and mode=='off':
    original['fee']=str(fee+1)
result=dict(schema='svm-simulate-m6-executor-output/v1',caseId=fixture['caseId'],executorSourceId=fixture['runtime']['binding']['executor']['id'],runtimeBindingHash=fixture['runtime']['binding']['bindingHash'],targetIndex=fixture['target']['index'],targetSignature=fixture['target']['signature'],prefix=prefix,omittedFeeEffects=[],original=original,replacement=original if fixture['target']['replacementTransactionBase64'] is not None else None,accountOverride=None)
tracked=any(row['requirement']=='tracked-generic-sysvar-context-v1' for row in fixture['runtime']['bankContext']['requiredRuntimeSysvars'])
response=dict(schema='svm-sysvar-discovery/v1',status='COMPLETE',reads=[],output=result) if tracked else result
if (root/'fail-requested').exists() and fixture['target']['replacementTransactionBase64'] is not None:
    response=dict(schema='svm-sysvar-discovery/v1',status='NEEDS_INPUT',pubkey='SysvarRent111111111111111111111111111111111',reads=[dict(pubkey='SysvarRent111111111111111111111111111111111',offset='0',length='8')])
if mode=='--capture':
    options=json.loads(pathlib.Path(sys.argv[4]).read_bytes())
    assert options['debug_port'] is None
    if (root/'mutate-options').exists():
        path=pathlib.Path(sys.argv[4]);path.chmod(0o600)
        path.write_bytes(path.read_bytes()+b' ')
    def raw(balance):
        return [dict(pubkey=account['pubkey'],state=dict(lamports=str(balance),owner=account['owner'],executable=account['executable'],rent_epoch=account['rentEpoch'],data=list(base64.b64decode(account['dataBase64']))))]
    events=[dict(kind='transaction_begin',accounts=raw(balance)),dict(kind='transaction_commit',outcome=dict(status='success'),accounts=raw(balance-fee))]
    journal=dict(signatures=[fixture['target']['signature']],scope='toy fee-only target',before_scope='tracked account',before_keys=[account['pubkey']],terminal_scope='committed tracked account',terminal_keys=[account['pubkey']],journal=dict(disposition='COMPLETE',reason=None,event_count=len(events),payload_json=json.dumps(events)))
    response=dict(schema='svm-historical-capture-worker-experimental/v1',status='COMPLETE',execution_mode='jit' if platform.machine()=='x86_64' else 'interpreter',output=response,captures=[],journals=[journal])
pathlib.Path(sys.argv[2]).write_text(json.dumps(response))
"#;

pub(super) fn fixture(root: &Path) -> (PreparedRequest, ResolvedWorker, ResolvedCaptureWorker) {
    let (mut request, reference) = fee_ledger::fixture(root, WORKER);
    let binary = root.join("capture");
    fs::write(
        &binary,
        format!("{WORKER}\n# separately pinned observation role\n"),
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let sha = file_sha256(&binary).unwrap();
    let mut descriptor = reference.descriptor.clone();
    descriptor.file = "capture".into();
    descriptor.sha256 = Digest::new(&sha).unwrap();
    descriptor.capabilities.extend([
        "runtime-full-capture-cli/v1".into(),
        "runtime-call-journal/v1".into(),
        "runtime-interpreter-capture/v1".into(),
    ]);
    let mut clock = [0u8; 40];
    clock[..8].copy_from_slice(&20u64.to_le_bytes());
    request.fixture["clock"] = json!({"pubkey":"SysvarC1ock11111111111111111111111111111111","sourceSlot":20,"role":"sysvar","presence":"present",
        "owner":"Sysvar1111111111111111111111111111111111111","executable":false,"lamports":"1","rentEpoch":"18446744073709551615","dataBase64":STANDARD.encode(clock)});
    let context_hash = Digest::of(b"synthetic trace test context");
    request.fixture["runtime"]["lastRestartSlot"] = json!({"bindingHash":context_hash});
    request.fixture["runtime"]["bankContext"] = json!({"targetSlot":20,"parentSlot":19,"lamportsPerSignature":5,"executionBlockhash":"synthetic",
        "sourceKind":"synthetic","blockSourceHash":request.block_sha256,"blockLineageHash":context_hash,"feeWitnessCount":0,
        "feeWitnessIndicesHash":Digest::of(b"[]"),"feeWitnessHash":Digest::of(b"[]"),"requiredRuntimeSysvars":[]});
    let inputs =
        svm_replay_engine::_2_prepare_state::discovery::initial_inputs(&request.fixture).unwrap();
    request.fixture =
        svm_replay_engine::_2_prepare_state::discovery::tracked_fixture(&request.fixture, &inputs)
            .unwrap();
    let capture = ResolvedCaptureWorker {
        worker: ResolvedWorker {
            descriptor,
            spec: WorkerSpec {
                executable: binary,
                sha256: sha,
            },
        },
        reference_sha256: reference.descriptor.sha256.clone(),
        gate_sha256: Digest::of(b"explicit protocol-only fixture gate"),
    };
    (request, reference, capture)
}

pub(super) fn policy() -> CaptureRequest {
    let mode = if cfg!(target_arch = "x86_64") {
        "jit"
    } else {
        "interpreter"
    };
    CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":mode,"level":"calls",
    "sbpfObservations":"none","filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":1048576,"maxEvents":100,"timeoutMs":10000}})).unwrap()).unwrap()
}

pub(super) fn run(
    root: &Path,
    request: &PreparedRequest,
    reference: &ResolvedWorker,
    capture: &ResolvedCaptureWorker,
) -> Result<svm_replay_engine::_4_simulate::trace::TraceExecution, svm_replay_protocol::Error> {
    execute(
        request,
        reference,
        capture,
        TraceContext {
            transport: &WorkerTransport::new(root),
            scratch_root: root,
            implementation_sha256: &Digest::of(b"test engine"),
        },
        &policy(),
        &ProducerBounds {
            register_rows: 0,
            memory_rows: None,
            max_invocations: 64,
        },
        &WorkerLimits::default(),
        &ExecutionBudget::new(Duration::from_secs(10), CancellationToken::new()).unwrap(),
    )
}
