"""Bounded source assessment: no product changes, provider calls or publication."""
import hashlib
import json
import os
import platform
import shutil
import signal
import subprocess
import sys
import tarfile
import time
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent


def first_blocker(run, stages):
    for stage in stages:
        if not run(*stage):
            return stage[0]
    return None


def assessment_host(operating_system, architecture):
    return operating_system in {'darwin', 'win32'} or (
        operating_system == 'linux' and architecture in {'aarch64', 'arm64'})


def safe_members(archive):
    members = archive.getmembers()
    for member in members:
        path = PurePosixPath(member.name)
        if path.is_absolute() or '..' in path.parts or not (member.isfile() or member.isdir()):
            raise ValueError(f'unsafe archive member: {member.name}')
    return members


def command(argv, cwd, log, seconds, env):
    started = time.monotonic()
    with log.open('xb') as stream:
        proc = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stream,
                                stderr=subprocess.STDOUT, start_new_session=os.name != 'nt',
                                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0)
        try:
            code = proc.wait(timeout=seconds)
            status = 'PASS' if code == 0 else 'FAIL'
        except subprocess.TimeoutExpired:
            if os.name == 'nt':
                subprocess.run(['taskkill', '/PID', str(proc.pid), '/T', '/F'],
                               stdout=stream, stderr=subprocess.STDOUT, check=True)
                proc.wait()
            else:
                os.killpg(proc.pid, signal.SIGTERM)
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, signal.SIGKILL)
                    proc.wait()
            code, status = proc.returncode, 'TIMEOUT'
    return {'status': status, 'exitCode': code,
            'elapsedSeconds': round(time.monotonic() - started, 3), 'log': log.name}


def main():
    if not assessment_host(sys.platform, platform.machine()):
        raise RuntimeError('Assessment requires Mac, Windows or Linux ARM64 host')
    reports = ROOT / 'reports'
    reports.mkdir()
    work = ROOT / 'work'
    work.mkdir(exist_ok=True)
    summary = {'status': 'IN_PROGRESS', 'host': {'os': platform.platform(),
               'arch': platform.machine(), 'python': sys.version}, 'inputs': {}, 'checks': [],
               'historicalReplayTested': False, 'nativeReplaySupportClaimed': False}

    def save():
        (reports / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')

    save()
    for line in (ROOT / 'inputs/SHA256SUMS').read_text().splitlines():
        expected, name = line.split()
        data = (ROOT / 'inputs' / name).read_bytes()
        actual = hashlib.sha256(data).hexdigest()
        if expected != actual:
            raise RuntimeError(f'archive hash mismatch: {name}')
        summary['inputs'][name] = actual
    for filename, destination in [('source.tar.gz', 'source'), ('worker-sources.tar.gz', 'workers')]:
        target = work / destination
        target.mkdir()
        with tarfile.open(ROOT / 'inputs' / filename) as archive:
            archive.extractall(target, members=safe_members(archive), filter='data')
    workers = work / 'workers/worker-sources'
    manifest = json.loads((workers / 'release.json').read_text())
    for row in manifest['files']:
        candidate = workers / row['path']
        if not candidate.resolve().is_relative_to(workers.resolve()):
            raise RuntimeError('worker manifest path escapes source root')
        if hashlib.sha256(candidate.read_bytes()).hexdigest() != row['sha256']:
            raise RuntimeError(f'worker source mismatch: {row["path"]}')
    consumer = work / 'consumer'
    consumer.mkdir()
    (consumer / 'package.json').write_text(json.dumps({'private': True, 'type': 'module'}))
    env = dict(os.environ, CARGO_TARGET_DIR=str(work / 'target'))
    deadline = time.monotonic() + (55 * 60 if '--launch' in sys.argv else (19 * 60 if sys.platform == 'darwin' else 10 * 60))

    def run(name, argv, cwd, timeout):
        remaining = deadline - time.monotonic()
        if remaining < 5:
            result = {'status': 'NOT_RUN_BUDGET', 'name': name}
        else:
            result = command(argv, cwd, reports / f'{name}.log', min(timeout, remaining), env)
            result['name'] = name
        summary['checks'].append(result)
        save()
        print(json.dumps(result), flush=True)
        return result['status'] == 'PASS'

    npm = shutil.which('npm')
    if npm is None:
        raise RuntimeError('Node/npm prerequisite missing')
    if '--launch' in sys.argv:
        executable = work / 'target/debug' / ('svm-replay.exe' if os.name == 'nt' else 'svm-replay')
        stages = [
            ('environment', ['rustc', '+1.94.0', '-vV'], ROOT, 30),
            ('coordinator-build', ['cargo', '+1.94.0', 'build', '--locked', '--jobs', '2',
                                   '-p', 'svm-replay', '--bin', 'svm-replay'], work / 'source', 420),
            ('cli-launch', [str(executable), '--help'], work / 'source', 30),
            ('bundle-tests', ['cargo', '+1.94.0', 'test', '--locked', '--jobs', '2',
                              '-p', 'svm-replay', '--bin', 'svm-replay', 'bundle::tests'], work / 'source', 120),
            ('reference-worker-build', ['cargo', '+1.94.0', 'build', '--locked', '--jobs', '2',
                                       '--manifest-path', str(workers / 'reference/executors/v2-3/Cargo.toml'),
                                       '--bin', 'm6-executor-v2-3'], workers, 2400),
            ('historical-worker-smoke', [sys.executable, str(ROOT / 'scripts/worker_smoke.py'),
                                         str(work / 'target/debug' / ('m6-executor-v2-3.exe' if os.name == 'nt' else 'm6-executor-v2-3')),
                                         str(ROOT / 'inputs'), str(reports / 'historical-smoke')], ROOT, 150),
            ('npm-install', [npm, 'install', '--ignore-scripts', '--no-audit', '--no-fund',
                             str(ROOT / 'inputs/sdk.tgz')], consumer, 60),
            ('sdk-open', ['node', str(ROOT / 'scripts/open.mjs'),
                          str(consumer / 'node_modules/@xternet/svm-replay/dist/index.js')], consumer, 30),
        ]
        if '--transport' in sys.argv:
            stages = stages[:4] + [('native-cli-tests', ['cargo', '+1.94.0', 'test', '--locked', '--jobs', '2',
                                   '-p', 'svm-replay', '--bin', 'svm-replay'], work / 'source', 180),
                                   ('native-transport-tests', ['cargo', '+1.94.0', 'test', '--locked',
                                   '--jobs', '2', '--test', 'windows_transport', '--', '--nocapture'], work / 'source', 180)]
        blocker = first_blocker(run, stages)
        summary['firstBlocker'] = blocker
        summary['status'] = 'BLOCKED' if blocker else 'LAUNCH_ONLY_NOT_REPLAY_QUALIFIED'
        save()
        return 1 if blocker else 0
    run('environment', ['rustc', '+1.94.0', '-vV'], ROOT, 30)
    sdk_installed = run('npm-install', [npm, 'install', '--ignore-scripts', '--no-audit', '--no-fund',
                                      str(ROOT / 'inputs/sdk.tgz')], consumer, 120)
    if sdk_installed:
        run('sdk-expected-rejection', ['node', str(ROOT / 'scripts/sdk.mjs'),
            str(consumer / 'node_modules/@xternet/svm-replay/dist/index.js')], consumer, 30)
    else:
        summary['checks'].append({'name': 'sdk-expected-rejection', 'status': 'NOT_RUN_SETUP_FAILED'})
        save()
    cargo = ['cargo', '+1.94.0', 'check', '--locked', '--jobs', '2']
    run('coordinator-all-targets', cargo + ['--workspace', '--all-targets'], work / 'source', 180)
    run('reference-v2-3', cargo + ['--manifest-path', str(workers / 'reference/executors/v2-3/Cargo.toml'),
                                 '--bin', 'm6-executor-v2-3'], workers, 180)
    run('capture-v4-2', cargo + ['--manifest-path', str(workers / 'capture/project/poc/m13-tracing/workers/v4-2-full/Cargo.toml'),
                               '--bin', 'm13-capture-worker-v4-2-full'], workers, 180)
    summary['status'] = 'CHECKS_PASSED_NOT_RUNTIME_QUALIFIED' if all(
        row['status'] == 'PASS' for row in summary['checks']) else 'BLOCKERS_OBSERVED'
    save()
    return 0 if summary['status'] == 'CHECKS_PASSED_NOT_RUNTIME_QUALIFIED' else 1


if __name__ == '__main__':
    raise SystemExit(main())
