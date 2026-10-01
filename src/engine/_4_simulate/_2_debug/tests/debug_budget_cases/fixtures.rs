use super::*;

// Computes a fee-only protocol fixture with zero guest instructions. The real
// worker process delays model bounded gate/session work, not historical SVM proof.
pub(super) const WORKER: &str = r#"#!/usr/bin/python3
import base64,hashlib,json,os,pathlib,platform,sys,time
fixture=json.loads(pathlib.Path(sys.argv[1]).read_bytes());root=pathlib.Path(__file__).parent
mode=sys.argv[3] if len(sys.argv)>3 else 'baseline'
options=json.loads(pathlib.Path(sys.argv[4]).read_bytes()) if mode=='--capture' else None
live=options is not None and options['debug_port'] is not None
with (root/'calls').open('a') as log:log.write(('live' if live else mode)+'\n')
if live:(root/'live-started').write_text(str(os.getpid()))
time.sleep((2.0 if (root/'stall-live').exists() else 0.5) if live else 0.2)
account=fixture['accounts'][0];fee=int(fixture['runtime']['bankContext']['lamportsPerSignature'])
def state(balance):
    return dict(presence='present',lamports=str(balance),owner=account['owner'],executable=account['executable'],rentEpoch=account['rentEpoch'],dataHash=hashlib.sha256(base64.b64decode(account['dataBase64'])).hexdigest(),tokenAmount=None)
def run(balance,end=False):
    return dict(status='ok',normalizedError=None,logs=[],computeUnits=0,fee=str(fee),returnData=None,accountTransitions=[dict(pubkey=account['pubkey'],before=state(balance),after=state(balance-fee))],endAccountStates=[dict(pubkey=account['pubkey'],state=state(balance-fee))] if end else [])
balance=int(account['lamports']);prefix=[]
for index,signature in zip(fixture['target']['prefixIndices'],fixture['target']['prefixSignatures']):
    prefix.append(dict(index=index,signature=signature,**run(balance)));balance-=fee
output=dict(schema='svm-simulate-m6-executor-output/v1',caseId=fixture['caseId'],executorSourceId=fixture['runtime']['binding']['executor']['id'],runtimeBindingHash=fixture['runtime']['binding']['bindingHash'],targetIndex=fixture['target']['index'],targetSignature=fixture['target']['signature'],prefix=prefix,omittedFeeEffects=[],original=run(balance,True),replacement=None,accountOverride=None)
response=dict(schema='svm-sysvar-discovery/v1',status='COMPLETE',reads=[],output=output)
if options is not None:
    def raw(balance):return [dict(pubkey=account['pubkey'],state=dict(lamports=str(balance),owner=account['owner'],executable=account['executable'],rent_epoch=account['rentEpoch'],data=list(base64.b64decode(account['dataBase64']))))]
    events=[dict(kind='transaction_begin',accounts=raw(balance)),dict(kind='transaction_commit',outcome=dict(status='success'),accounts=raw(balance-fee))]
    journal=dict(signatures=[fixture['target']['signature']],scope='fee-only protocol fixture',before_scope='tracked account',before_keys=[account['pubkey']],terminal_scope='committed tracked account',terminal_keys=[account['pubkey']],journal=dict(disposition='COMPLETE',reason=None,event_count=len(events),payload_json=json.dumps(events)))
    native='jit' if platform.machine()=='x86_64' else 'interpreter'
    response=dict(schema='svm-historical-capture-worker-experimental/v1',status='COMPLETE',execution_mode='interpreter-debug' if live else native,output=response,captures=[],journals=[journal])
pathlib.Path(sys.argv[2]).write_text(json.dumps(response))
"#;

pub(super) fn fixture(root: &Path) -> (PreparedRequest, ResolvedWorker, ResolvedCaptureWorker) {
    let (mut request, reference) = fee_ledger::fixture(root, WORKER);
    let binary = root.join("capture");
    fs::write(
        &binary,
        format!("{WORKER}\n# distinct pinned capture role\n"),
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
        "runtime-interpreter-debug/v1".into(),
        "runtime-interpreter-capture/v1".into(),
    ]);
    let mut clock = [0u8; 40];
    clock[..8].copy_from_slice(&20u64.to_le_bytes());
    request.fixture["clock"] = json!({"pubkey":"SysvarC1ock11111111111111111111111111111111","sourceSlot":20,"role":"sysvar","presence":"present","owner":"Sysvar1111111111111111111111111111111111111","executable":false,"lamports":"1","rentEpoch":"18446744073709551615","dataBase64":STANDARD.encode(clock)});
    let hash = Digest::of(b"debug budget protocol fixture context");
    request.fixture["runtime"]["lastRestartSlot"] = json!({"bindingHash":hash});
    request.fixture["runtime"]["bankContext"] = json!({"targetSlot":20,"parentSlot":19,"lamportsPerSignature":5,"executionBlockhash":"synthetic","sourceKind":"synthetic","blockSourceHash":request.block_sha256,"blockLineageHash":hash,"feeWitnessCount":0,"feeWitnessIndicesHash":Digest::of(b"[]"),"feeWitnessHash":Digest::of(b"[]"),"requiredRuntimeSysvars":[]});
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
        gate_sha256: Digest::of(b"budget fixture gate"),
    };
    (request, reference, capture)
}

pub(super) fn run(
    root: &Path,
    job_ms: u64,
    capture_ms: u64,
    cancel: CancellationToken,
) -> Result<svm_replay_engine::_4_simulate::debug::DebugExecution, svm_replay_protocol::Error> {
    let (request, reference, capture) = fixture(root);
    let (_controller, mut driver) = channel();
    let policy=CaptureRequest::parse(&serde_json::to_vec(&json!({"schema":"svm-capture-request/v2","executionMode":"interpreter-debug","level":"calls","sbpfObservations":"none","filter":{"programIds":[],"instructionIndices":[]},"limits":{"maxBytes":1048576,"maxEvents":100,"timeoutMs":capture_ms}})).unwrap()).unwrap();
    execute(
        &request,
        &reference,
        &capture,
        TraceContext {
            transport: &WorkerTransport::new(root),
            scratch_root: root,
            implementation_sha256: &Digest::of(b"explicit budget fixture"),
        },
        &policy,
        &ProducerBounds {
            register_rows: 0,
            memory_rows: None,
            max_invocations: 64,
        },
        &WorkerLimits::default(),
        &ExecutionBudget::new(Duration::from_millis(job_ms), cancel).unwrap(),
        &mut driver,
    )
}
