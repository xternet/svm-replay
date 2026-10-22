// Representative whole-tool qualification; complete Linux regression is separate.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {cpSync, readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {createRequire} from 'node:module';

const work = resolve(process.env.TOOL_WORK);
const workspace = join(work, 'toy/workspace');
const require = createRequire(join(workspace, 'package.json'));
const {Keypair} = require('@solana/web3.js');
const rows = [];
function run(name, command, args, cwd = workspace, env = process.env) {
  const result = spawnSync(command, args, {cwd, env, encoding: 'utf8', timeout: 420000,
    maxBuffer: 32 << 20});
  writeFileSync(join(work, 'reports', name + '.log'), String(result.stdout) + '\n' + String(result.stderr));
  rows.push({name, status: result.status === 0 ? 'PASS' : 'FAIL', exitCode: result.status});
  if (result.error) throw result.error;
  assert.equal(result.status, 0, name + '\n' + String(result.stderr).slice(-3000));
  return result.stdout;
}
let failure;
try {
  if (process.platform === 'darwin') {
    // sandbox-exec is inherited by every child, including the browser/backend.
    const denied = spawnSync('python3', ['-c',
      'import socket\ns=socket.socket();s.settimeout(2)\ntry:s.connect(("1.1.1.1",443))\nexcept PermissionError:print("NETWORK_DENIED")\nelse:raise RuntimeError("network guard absent")'],
      {encoding: 'utf8'});
    assert.equal(denied.status, 0, denied.stderr);
    assert.match(denied.stdout, /NETWORK_DENIED/);
    const path = join(workspace, 'tests/ci.ts');
    const original = readFileSync(path, 'utf8');
    const guard = 'assert.deepEqual(Object.keys(networkInterfaces()), ["lo"], "Provider networking must be disabled");';
    assert.equal(original.split(guard).length, 2);
    writeFileSync(path, original.replace(guard,
      'assert.equal(process.env.TOOL_NETWORK_GUARD, "macos-sandbox-loopback-only");'));
    process.env.TOOL_NETWORK_GUARD = 'macos-sandbox-loopback-only';
    rows.push({name: 'inherited-network-sandbox', status: 'PASS',
      adaptation: 'Only Linux interface-name assertion replaced; actual external connect denied by macOS sandbox'});
  }
  process.env.SVM_REPLAY_BIN = join(work, 'installed/bin/svm-replay');
  process.env.API_ALCHEMY = 'offline-fixture-not-a-key';
  if (!process.env.TOOL_BROWSER_ONLY) {
    mkdirSync(join(workspace, '.surfpool/history'), {recursive: true});
    cpSync(join(work, 'fixture/svm-replay-smoke-413023260/data/cache'),
      join(workspace, '.surfpool/history/cache'), {recursive: true});
    writeFileSync(join(workspace, 'wallet.json'), JSON.stringify([...Keypair.generate().secretKey]));
    run('anchor-typecheck', join(workspace, 'node_modules/.bin/tsc'), ['--noEmit']);
    const output = run('anchor-historical', process.env.ANCHOR_BIN,
      ['test', '--skip-build', '--skip-deploy']);
    assert.match(output, /2 passing/);
    const env = {...process.env, M59_AFTER: '413023260', M59_SLOT: '413023261',
      M59_SVM_REPLAY_BIN: process.env.SVM_REPLAY_BIN,
      M59_HISTORICAL_DATA_DIR: join(work, 'fixture/svm-replay-smoke-413023260/data'),
      M59_HISTORICAL_CONFIG: join(work, 'fixture/svm-replay-smoke-413023260/Surfpool.toml')};
    const outputSdk = run('surfpool-rust-sdk', process.env.HISTORICAL_TEST_BINARY,
      ['--ignored', '--test-threads=1', '--nocapture'], workspace, env);
    assert.match(outputSdk, /3 passed; 0 failed; 0 ignored/);
  }
  run('studio-browser', process.execPath, [new URL('browser.mjs', import.meta.url).pathname]);
} catch (error) {failure = error;}
writeFileSync(join(work, 'reports/summary.json'), JSON.stringify({
  status: failure ? 'FAIL' : 'PASS', platform: process.platform, arch: process.arch,
  kernel: process.env.TOOL_WSL_KERNEL, rows, error: failure ? String(failure) : null,
  scope: 'Selected native bytes; original Anchor two tests, Surfpool SDK three and rendered Studio workflows. No live provider acquisition.',
}, null, 2));
if (failure) throw failure;
