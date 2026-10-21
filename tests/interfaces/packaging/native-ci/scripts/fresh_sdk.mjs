// Consumer proof: import the installed tarball; execute the downloaded native bundle.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync, writeFileSync} from 'node:fs';
import {dirname, join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const [sdkPath, installation, output] = process.argv.slice(2).map(p => resolve(p));
const {Replay, verifyArtifact} = await import(pathToFileURL(sdkPath).href);
const read = p => JSON.parse(readFileSync(p));
const sha = p => createHash('sha256').update(readFileSync(p)).digest('hex');
const bundlePath = join(installation, 'bundle.json');
const bundleSha256 = sha(bundlePath);
await assert.rejects(Replay.open({bundlePath, bundleSha256: '0'.repeat(64)}),
  error => error.code === 'BUNDLE_INTEGRITY');
const replay = await Replay.open({bundlePath, bundleSha256, dataDir: join(output, 'fresh-sdk-data')});
const doctor = await replay.capabilities();
assert.equal(doctor.outcome, 'COMPLETED');
assert.equal(doctor.workers.length, 7);
assert.equal(doctor.captureWorkers.length, 7);
const demo = read(join(installation, 'demo/manifest.json'));
const {receipt} = await replay.simulate({request: {path: join(installation, 'demo/request.json')}, timeoutMs: 600000});
assert.equal(receipt.outcome, 'COMPLETED', JSON.stringify(receipt));
assert.equal(receipt.currentStateFallback, false);
assert.equal(receipt.verification.status, 'PASS');
assert.equal(receipt.controlVerification.status, 'PASS');
assert.equal(receipt.output.sha256, demo.expectedOutputSha256);
const artifact = await verifyArtifact(dirname(receipt.receiptPath), receipt.output, 256 * 1024 * 1024);
assert.equal(sha(artifact.path), demo.expectedOutputSha256);
writeFileSync(join(output, 'fresh-sdk-summary.json'), JSON.stringify({status: 'PASS',
  sdkPath, bundleSha256, receiptPath: receipt.receiptPath, outputSha256: sha(artifact.path),
  nativeArchitecture: process.arch, providerCalls: false, negativeIntegrityCheck: 'PASS'}, null, 2) + '\n');
