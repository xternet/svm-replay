use super::*;

// A real executable implementing a deliberately small fee-only ledger. It
// computes checkpoint state and outputs from input accounts; it is not SVM
// and never supplies historical parity evidence.
pub(super) const WORKER: &str = r#"#!/usr/bin/python3
import base64, copy, hashlib, json, pathlib, sqlite3, sys
fixture = json.loads(pathlib.Path(sys.argv[1]).read_bytes())
output_path, mode, checkpoint_path = map(pathlib.Path, [sys.argv[2], sys.argv[3], sys.argv[4]])
mode = str(mode)
root = pathlib.Path(__file__).parent
with (root / 'calls').open('a') as log:
    log.write(mode + '\n')
def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()
def digest(value):
    return hashlib.sha256(value).hexdigest()
canonical = copy.deepcopy(fixture)
canonical.pop('accountOverride')
canonical.pop('requestedAccountOverrides', None)
canonical['target'].pop('replacementTransactionBase64')
account = fixture['accounts'][0]
fee = int(fixture['runtime']['bankContext']['lamportsPerSignature'])
def state(balance):
    return dict(presence='present', lamports=str(balance), owner=account['owner'], executable=account['executable'], rentEpoch=account['rentEpoch'], dataHash=digest(base64.b64decode(account['dataBase64'])), tokenAmount=None)
def execute(balance, end=False):
    after = balance - fee
    return dict(status='ok', normalizedError=None, logs=[], computeUnits=0, fee=str(fee), returnData=None, accountTransitions=[dict(pubkey=account['pubkey'], before=state(balance), after=state(after))], endAccountStates=[dict(pubkey=account['pubkey'], state=state(after))] if end else [])
metrics = dict(prefixTransactionsExecuted=0, prefixSimulationCalls=0, prefixCommitCalls=0, prefixTransactionsReused=0)
if mode == '--checkpoint-create':
    balance = int(account['lamports'])
    prefix = []
    for index, signature in zip(fixture['target']['prefixIndices'], fixture['target']['prefixSignatures']):
        prefix.append(dict(index=index, signature=signature, **execute(balance)))
        balance -= fee
        metrics['prefixTransactionsExecuted'] += 1
        metrics['prefixSimulationCalls'] += 1
        metrics['prefixCommitCalls'] += 1
    checkpoint = dict(schema='svm-m10-boundary/v1', fixtureSha256=digest(encoded(canonical)), workerSha256=digest(pathlib.Path(__file__).read_bytes()), image=dict(balance=balance), prefix=prefix, omittedFeeEffects=[])
    raw = encoded(checkpoint)
    with checkpoint_path.open('xb') as destination:
        destination.write(raw)
    response = dict(status='CREATED', checkpointSha256=digest(raw), checkpointBytes=len(raw))
else:
    raw = checkpoint_path.read_bytes()
    assert digest(raw) == sys.argv[5], 'checkpoint byte identity'
    checkpoint = json.loads(raw)
    assert checkpoint['fixtureSha256'] == digest(encoded(canonical)), 'fixture identity'
    metrics['prefixTransactionsReused'] = len(checkpoint['prefix'])
    original = execute(checkpoint['image']['balance'], True)
    result = dict(schema='svm-simulate-m6-executor-output/v1', caseId=fixture['caseId'], executorSourceId=fixture['runtime']['binding']['executor']['id'], runtimeBindingHash=fixture['runtime']['binding']['bindingHash'], targetIndex=fixture['target']['index'], targetSignature=fixture['target']['signature'], prefix=checkpoint['prefix'], omittedFeeEffects=[], original=original, replacement=original if fixture['target']['replacementTransactionBase64'] is not None else None, accountOverride=None)
    response = dict(status='EXECUTED', checkpointSha256=digest(raw), output=result)
    if (root / 'mutate-checkpoint').exists():
        checkpoint_path.write_bytes(raw + b' ')
    if (root / 'release-lease').exists():
        connection = sqlite3.connect(root / 'store' / 'index.sqlite')
        connection.execute('UPDATE leases SET released=1')
        connection.commit()
        connection.close()
response.update(schema='svm-m10-checkpoint-output/v1', metrics=metrics, reads=[])
missing = root / 'needs-input'
if missing.exists() and (missing.read_text() == 'all' or fixture['target']['replacementTransactionBase64'] is not None):
    response.pop('output', None)
    response.update(status='NEEDS_INPUT', guardedExecutionAttempted=True, certifiedResult=False, pubkey='SysvarRent111111111111111111111111111111111', reads=[dict(pubkey='SysvarRent111111111111111111111111111111111', offset='0', length='8')])
output_path.write_bytes(encoded(response))
"#;

pub(super) fn fixture(root: &Path) -> (PreparedRequest, ResolvedWorker) {
    super::super::fee_ledger::fixture(root, WORKER)
}

pub(super) fn run(
    root: &Path,
    store: &mut Store,
    request: &PreparedRequest,
    worker: &ResolvedWorker,
    result_cache: bool,
) -> Result<serde_json::Value, Error> {
    execute(
        request,
        worker,
        CacheContext {
            store,
            transport: &WorkerTransport::new(root),
            scratch_root: root,
            catalog_sha256: &Digest::of(b"test catalog"),
            implementation_sha256: &Digest::of(b"test native implementation"),
        },
        &WorkerLimits::default(),
        &ExecutionBudget::new(Duration::from_secs(10), CancellationToken::new()).expect("budget"),
        CacheOptions { result_cache },
    )
}
