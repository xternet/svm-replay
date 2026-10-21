"""Native validation of a declared seven-family selection; outputs stay pinned."""
import hashlib
import json
import os
import platform
import shutil
import sys
import tarfile
import re
from pathlib import Path
from probe import command, safe_members
from release_inputs import require_families, release_version, rebind_request
from native_host import require_native_host, require_binary_arch
from build_paths import build_environment

ROOT = Path(__file__).resolve().parent.parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def verify_native_output(raw, receipt_pin, native_binding, original_binding, expected_pin):
    if digest(raw) != receipt_pin:
        raise RuntimeError('native receipt output digest differs')
    output = json.loads(raw)
    if output['runtimeBindingHash'] != native_binding:
        raise RuntimeError('native output binding differs')
    output['runtimeBindingHash'] = original_binding
    if digest(encoded(output)) != expected_pin:
        raise RuntimeError('historical execution output differs')


def main():
    coordinator = Path(os.environ['SVM_REPLAY_VALIDATION_SOURCE']).resolve(strict=True)
    native_os = {'linux': 'linux', 'darwin': 'macos', 'win32': 'windows'}[sys.platform]
    native_arch = {'x86_64': 'x86_64', 'aarch64': 'aarch64', 'arm64': 'aarch64',
                   'AMD64': 'x86_64', 'ARM64': 'aarch64'}[platform.machine()]
    work = ROOT / 'work'
    work.mkdir(exist_ok=True)
    reports = ROOT / 'reports'
    reports.mkdir()
    scope = os.environ['NATIVE_SCOPE']
    if scope != 'all':
        raise RuntimeError('public native qualification requires the complete all scope')
    summary = {'status': 'IN_PROGRESS', 'scope': scope, 'host': platform.platform(), 'checks': [], 'cases': []}
    env = build_environment(coordinator, work)

    def save():
        (reports / 'summary.json').write_bytes(encoded(summary))

    def run(name, args, cwd, seconds=900):
        result = command(args, cwd, reports / (name + '.log'), seconds, env)
        result['name'] = name
        summary['checks'].append(result)
        save()
        print(json.dumps(result), flush=True)
        if result['status'] != 'PASS':
            print((reports / (name + '.log')).read_text(errors='replace')[-12000:], flush=True)
            raise RuntimeError(f'{name}: {result["status"]}')
        return reports / (name + '.log')

    try:
        for line in (ROOT / 'inputs/SHA256SUMS').read_text().splitlines():
            pin, name = line.split()
            if digest((ROOT / 'inputs' / name).read_bytes()) != pin:
                raise RuntimeError(f'input pin mismatch: {name}')
        commit = run('source-commit', ['git', 'rev-parse', 'HEAD'], coordinator, 30).read_text().strip()
        if commit != os.environ['GITHUB_SHA']:
            raise RuntimeError('source checkout differs from the dispatched commit')
        run('source-clean', ['git', 'diff', '--exit-code', 'HEAD', '--'], coordinator, 30)
        run('repository-layout', ['node', '--test', 'tests/interfaces/layout/repository.test.mjs'], coordinator)
        summary['sourceCommit'] = commit
        for name, dest in [('worker-sources.tar.gz', 'workers'),
                           ('coordinator-licenses.tar.gz', 'licenses')]:
            target = work / dest
            target.mkdir()
            with tarfile.open(ROOT / 'inputs' / name) as archive:
                archive.extractall(target, members=safe_members(archive), filter='data')
            # Cargo's reused dep-info is timestamp-based. Tar preserves older
            # source mtimes, which can otherwise admit stale coordinator bytes.
            # Refresh timestamps only; source content/pins remain unchanged.
            paths = target.rglob('*')
            for path in paths:
                if path.is_file() and path.suffix in {'.rs', '.toml'}:
                    os.utime(path, None)
        fixture_archive = ROOT / 'inputs/qualification.tar.gz'
        with tarfile.open(fixture_archive) as archive:
            archive.extractall(work / 'fixtures', members=safe_members(archive), filter='data')
        fixtures = work / 'fixtures/inputs'
        matrix = json.loads((fixtures / 'cases.json').read_bytes())
        selection = json.loads((ROOT / 'inputs/public-selection.json').read_bytes())
        if len(matrix['cases']) != 7 or matrix['cases'] != selection['cases']:
            raise RuntimeError('historical case selection differs from the declared seven cases')
        summary['selection'] = {'cases': selection['cases'], 'wholePrivateCorpus': False}
        workers = work / 'workers/worker-sources'
        manifest = json.loads((workers / 'release.json').read_bytes())
        require_families(matrix['cases'], [b['family'] for b in manifest['builds']])
        qualified = json.loads((ROOT / 'inputs/qualified-catalog.json').read_bytes())
        summary['releaseVersion'] = release_version(coordinator)
        summary['workerSourcesSha256'] = digest((ROOT / 'inputs/worker-sources.tar.gz').read_bytes())
        for row in manifest['files']:
            path = workers / row['path']
            if not path.resolve().is_relative_to(workers.resolve()) or digest(path.read_bytes()) != row['sha256']:
                raise RuntimeError(f'worker source differs: {row["path"]}')
        compiler = run('compiler', ['rustc', '+1.94.0', '-vV'], ROOT, 30)
        summary['nativeCompilerHost'] = require_native_host(
            compiler.read_text(), sys.platform, platform.machine(), os.environ['RUNNER_ARCH'])
        cargo = ['cargo', '+1.94.0', 'build', '--release', '--locked', '--jobs', '4']
        summary['workerBuildProfile'] = 'release'
        run('cli-build', cargo + ['-p', 'svm-replay', '--bin', 'svm-replay'], coordinator)
        suffix = '.exe' if os.name == 'nt' else ''
        binary = work / ('target/release/svm-replay' + suffix)
        require_binary_arch(binary, native_os, native_arch)
        summary['binarySha256'] = digest(binary.read_bytes())
        run('launch', [str(binary), '--help'], ROOT, 30)
        run('workspace-tests', ['cargo', '+1.94.0', 'test', '--no-fail-fast', '--locked', '--jobs', '4',
                                   '--workspace'], coordinator, 600)
        run('signature-format-tests', ['cargo', '+1.94.0', 'test', '--locked', '--jobs', '4',
                                      '--test', 'signature_cli', '--test', 'pretty_cli'], coordinator, 300)
        run('trace-contract-tests', ['cargo', '+1.94.0', 'test', '--locked', '--jobs', '4',
                                     '-p', 'svm-replay-engine', '--test', 'trace'], coordinator, 180)
        run('owner-tests', ['cargo', '+1.94.0', 'test', '--locked', '--jobs', '4',
                            '--test', 'windows_transport' if os.name == 'nt' else 'worker_owner'], coordinator, 180)
        npm = shutil.which('npm')
        if npm is None:
            raise RuntimeError('npm unavailable')
        sdk_source = coordinator / 'src/bindings/typescript'
        run('sdk-dependencies', [npm, 'ci', '--ignore-scripts', '--no-audit', '--no-fund'], sdk_source, 180)
        run('sdk-unit-tests', [npm, 'test'], sdk_source, 180)
        sdk_packages = reports / 'sdk-packed'
        sdk_packages.mkdir()
        run('sdk-pack', [npm, 'pack', '--pack-destination', str(sdk_packages)], sdk_source, 180)
        sdk_tarball = sdk_packages / ('xternet-svm-replay-' + summary['releaseVersion'] + '.tgz')
        summary['sdkTarballSha256'] = digest(sdk_tarball.read_bytes())
        bundle = reports / 'native'
        bundle.mkdir()
        catalog = {'schema': 'svm-replay-workers/v1',
                   'platform': {'os': native_os, 'arch': native_arch},
                   'workers': []}
        if sys.platform == 'linux':
            catalog['platform']['glibcMin'] = '2.39'
        for build in manifest['builds']:
            if build['kind'] != 'reference':
                continue
            family = build['family']
            package = 'm6-executor-' + family
            native_target = work / 'native-target'
            run('build-' + family, cargo + ['--target-dir', str(native_target), '--manifest-path', str(workers / build['manifest']), '--bin', package], workers, 1800)
            native_worker = work / ('native-target/release/' + package + suffix)
            require_binary_arch(native_worker, native_os, native_arch)
            test_log = run('session-unit-' + family, ['cargo', '+1.94.0', 'test', '--release',
                '--locked', '--jobs', '4', '--target-dir', str(native_target), '--manifest-path',
                str(workers / build['manifest']), '--bin', package, 'm12_session',
                '--', '--test-threads=2'], workers, 900)
            census = re.search(r'test result: ok\. ([0-9]+) passed; 0 failed; ([0-9]+) ignored;', test_log.read_text())
            if census is None or int(census[1]) == 0:
                raise RuntimeError('native session unit census missing or empty: ' + family)
            summary.setdefault('workerSessionTests', []).append({'family': family,
                'passed': int(census[1]), 'ignored': int(census[2]), 'scope': 'synthetic native session tests'})
            worker_pin = digest(native_worker.read_bytes())
            rows = [row for row in matrix['cases'] if row['family'] == family]
            exemplar = json.loads((fixtures / rows[0]['file']).read_bytes())
            executor = exemplar['fixture']['runtime']['binding']['executor']
            build_pin = digest(encoded({'binarySha256': worker_pin, 'target': catalog['platform'],
                                        'compiler': digest((reports / 'compiler.log').read_bytes()),
                                        'workerSources': digest((ROOT / 'inputs/worker-sources.tar.gz').read_bytes())}))
            destination = bundle / (package + suffix)
            shutil.copy2(native_worker, destination)
            # Remove only this job's regenerable build cache, never source or evidence.
            if native_target.is_symlink() or native_target.parent != work:
                raise RuntimeError('native cache ownership differs')
            shutil.rmtree(native_target)
            reviewed = next(row for row in qualified['workers'] if row['family'] == family)
            descriptor = dict(reviewed, file=destination.name, sha256=worker_pin, buildHash=build_pin,
                              sourceSha256=build['portableInputSha256'])
            catalog['workers'].append(descriptor)
            catalog_path = bundle / 'catalog.json'
            catalog_path.write_bytes(encoded(catalog))
            for row in rows:
                raw = (fixtures / row['file']).read_bytes()
                if digest(raw) != row['requestSha256']:
                    raise RuntimeError('original request digest differs')
                request = json.loads(raw)
                original_binding_hash = request['fixture']['runtime']['binding']['bindingHash']
                request = rebind_request(request, descriptor)
                binding = request['fixture']['runtime']['binding']
                request_path = reports / (row['id'] + '.json')
                request_path.write_bytes(encoded(request))
                if scope not in {'corpus', 'all'}:
                    continue
                output = run('replay-' + row['id'], [str(binary), 'simulate', '--request', str(request_path),
                             '--catalog', str(catalog_path), '--catalog-sha256', digest(encoded(catalog)),
                             '--data-dir', str(reports / 'data')], ROOT, 600)
                receipt = json.loads(output.read_bytes())
                if receipt.get('outcome') != 'COMPLETED' or receipt.get('currentStateFallback') is not False:
                    raise RuntimeError(f'non-completed or fallback: {row["id"]}')
                if receipt['verification']['status'] != 'PASS' or receipt['controlVerification']['status'] != 'PASS':
                    raise RuntimeError('verification missing')
                actual = Path(receipt['receiptPath']).parent / receipt['output']['file']
                verify_native_output(actual.read_bytes(), receipt['output']['sha256'],
                                     binding['bindingHash'], original_binding_hash, row['outputSha256'])
                summary['cases'].append({'id': row['id'], 'status': 'PASS', 'originalRequestSha256': row['requestSha256'],
                                         'nativeRequestSha256': digest(request_path.read_bytes()), 'outputSha256': row['outputSha256']})
                save()
        if scope in {'corpus', 'all'} and len(summary['cases']) != len(matrix['cases']):
            raise RuntimeError('incomplete corpus selection')
        if scope in {'corpus', 'all'}:
            from native_sessions import qualify as qualify_sessions
            summary['sessions'] = qualify_sessions(
                ROOT / 'inputs/session', binary, catalog_path, coordinator,
                work / 'target', reports / 'native-sessions')
            save()
        run('bundle-pack', [str(binary), 'bundle', 'pack', '--catalog', str(catalog_path),
                           '--catalog-sha256', digest(encoded(catalog)), '--output', str(reports / 'bundle'),
                           '--reference-only'], ROOT, 180)
        consumer = work / 'consumer'
        consumer.mkdir()
        (consumer / 'package.json').write_bytes(encoded({'private': True, 'type': 'module'}))
        run('sdk-install', [npm, 'install', '--ignore-scripts', '--no-audit', '--no-fund',
                            str(sdk_tarball)], consumer, 120)
        if scope in {'capture', 'all'}:
            from native_capture import qualify
            qualify(run, cargo, work, reports, fixtures, workers, manifest, matrix, catalog, binary)
            run('full-bundle-pack', [str(binary), 'bundle', 'pack', '--catalog', str(catalog_path),
                '--catalog-sha256', digest(encoded(catalog)), '--output', str(reports / 'full-bundle')], ROOT, 180)
            run('sdk-six-traces', ['node', str(ROOT / 'scripts/native_sdk.mjs'),
                str(consumer / 'node_modules/@xternet/svm-replay/dist/index.js'),
                str(reports), str(fixtures), 'trace'], ROOT, 1800)
            summary['status'] = 'ALL_NATIVE_CHECKS_PASS' if scope == 'all' else 'SDK_TRACE_DEBUG_PASS'
            save()
            return 0
        run('sdk-six', ['node', str(ROOT / 'scripts/native_sdk.mjs'),
                        str(consumer / 'node_modules/@xternet/svm-replay/dist/index.js'),
                        str(reports), str(fixtures)], ROOT, 1800)
        summary['status'] = ('SDK_PASS_CAPTURE_PENDING' if scope == 'sdk'
                             else 'REFERENCE_CORPUS_AND_SDK_PASS_CAPTURE_PENDING')
        save()
        return 0
    except Exception as error:
        summary['status'] = 'BLOCKED'
        summary['error'] = str(error)
        save()
        raise


if __name__ == '__main__':
    raise SystemExit(main())
