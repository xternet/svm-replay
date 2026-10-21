// Private native reference qualification; no provider access or expected-output edits.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {join, dirname, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const [sdk, reportsPath, fixturesPath, mode = 'reference'] = process.argv.slice(2);
assert.ok(['reference', 'trace'].includes(mode));
const reports = resolve(reportsPath), fixtures = resolve(fixturesPath);
const {Replay, verifyArtifact} = await import(pathToFileURL(resolve(sdk)).href);
const read = path => JSON.parse(readFileSync(path));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const bundle = join(reports, mode === 'trace' ? 'full-bundle/bundle.json' : 'bundle/bundle.json');
const replay = await Replay.open({bundlePath: bundle, bundleSha256: hash(readFileSync(bundle)), dataDir: join(reports, 'sdk-data')});
const matrix = read(join(fixtures, 'cases.json'));
const results = [];
for (const family of [...new Set(matrix.cases.map(row => row.family))]) {
    const row = matrix.cases.find(row => row.family === family);
    const original = read(join(fixtures, row.file));
    const native = read(join(reports, row.id + '.json'));
    const options = mode === 'trace' ? {
        request: {path: join(reports, family + '-trace-request.json')},
        trace: {path: join(reports, family + '-trace-policy.json')},
    } : {request: {path: join(reports, row.id + '.json')}};
    const {receipt} = await replay.simulate({...options, timeoutMs: 600000});
    assert.equal(receipt.outcome, 'COMPLETED', JSON.stringify(receipt));
    assert.equal(receipt.currentStateFallback, false);
    assert.equal(receipt.verification.status, 'PASS');
    assert.equal(receipt.controlVerification.status, 'PASS');
    const artifact = await verifyArtifact(dirname(receipt.receiptPath), receipt.output, 256*1024*1024);
    const output = read(artifact.path);
    assert.equal(output.runtimeBindingHash, native.fixture.runtime.binding.bindingHash);
    output.runtimeBindingHash = original.fixture.runtime.binding.bindingHash;
    // Rust result files are recursively key-sorted canonical JSON; replacing the
    // existing binding value preserves ordering and all execution data.
    assert.equal(hash(JSON.stringify(output)), row.outputSha256);
    const exports = [];
    if (mode === 'trace') {
        assert.ok(receipt.trace.exports.length > 0);
        const expectedMode = read(join(reports, family + '-trace-policy.json')).capture.executionMode;
        for (const item of receipt.trace.exports) {
            assert.equal(item.artifact.executionMode, expectedMode);
            assert.ok(['COMPLETE', 'TRUNCATED'].includes(item.artifact.status));
            const handle = await verifyArtifact(dirname(receipt.receiptPath), item, 256*1024*1024);
            const bytes = readFileSync(handle.path);
            assert.equal(hash(bytes), item.artifact.payload.sha256);
            assert.equal(bytes.length, item.artifact.payload.bytes);
            assert.equal(JSON.parse(bytes).length, item.artifact.payload.events);
            exports.push({phase: item.phase, status: item.artifact.status, executionMode: expectedMode});
        }
    }
    results.push({family, caseId: row.id, status: 'PASS', receiptPath: receipt.receiptPath, exports});
    writeFileSync(join(reports, 'sdk-summary.json'), JSON.stringify(results, null, 2));
    console.log(JSON.stringify(results.at(-1)));
}
