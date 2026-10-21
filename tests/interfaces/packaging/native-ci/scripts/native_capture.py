"""Fresh native reference/off/capture gates followed by public CLI trace/debug."""
import json
import gc
import os
import shutil
from pathlib import Path
from native_matrix import ROOT, digest, encoded
from release_inputs import unwrap, rebind_request
from native_host import require_binary_arch


def qualify(run, cargo, work, reports, fixtures, workers, manifest, matrix, catalog, binary):
    original_catalog = json.loads((ROOT / 'inputs/original-catalog.json').read_bytes())
    if digest((ROOT / 'inputs/original-catalog.json').read_bytes()) != matrix['catalogSha256']:
        raise RuntimeError('original capture catalog pin differs')
    mode = 'jit' if catalog['platform']['arch'] == 'x86_64' and os.name != 'nt' else 'interpreter'
    traces = json.loads((fixtures / 'traces.json').read_bytes())
    catalog['captureWorkers'] = []
    catalog_path = reports / 'native/catalog.json'
    suffix = '.exe' if os.name == 'nt' else ''
    for build in manifest['builds']:
        if build['kind'] != 'capture':
            continue
        family = build['family']
        package = 'm13-capture-worker-' + family + '-full'
        native_target = work / 'native-target'
        run('capture-build-' + family, cargo + ['--target-dir', str(native_target), '--manifest-path', str(workers / build['manifest']), '--bin', package], workers, 1800)
        capture = reports / ('native/' + package + suffix)
        shutil.copy2(work / ('native-target/release/' + package + suffix), capture)
        require_binary_arch(capture, catalog['platform']['os'], catalog['platform']['arch'])
        if native_target.is_symlink() or native_target.parent != work:
            raise RuntimeError('capture cache ownership differs')
        shutil.rmtree(native_target)
        reference = next(row for row in catalog['workers'] if row['family'] == family)
        case = next(row for row in matrix['cases'] if row['family'] == family)
        trace = next(row for row in traces if row['caseId'] == case['id'])
        raw = (fixtures / trace['file']).read_bytes()
        if digest(raw) != trace['sha256']:
            raise RuntimeError('original trace input differs')
        request = json.loads(raw)
        # Functional qualification on slower hosts uses an explicit longer
        # deadline. Preserve the original inputs and record this timing-only change.
        original_request_timeout = request['limits']['timeoutMs']
        request['limits']['timeoutMs'] = 900000
        # Same binding already validated by SDK; retain trace-specific budgets.
        native = json.loads((reports / (case['id'] + '.json')).read_bytes())
        request['fixture']['runtime']['binding'] = native['fixture']['runtime']['binding']
        fixture_path = reports / (family + '-gate-fixture.json')
        fixture_path.write_bytes(encoded(request['fixture']))
        options = {'register_limit': 2000000, 'memory_limit': 2000000, 'program_ids': [],
                   'instruction_indices': [], 'debug_port': None,
                   'journal': {'max_events': 1000000, 'max_bytes': 268435456,
                               'program_ids': [], 'instruction_indices': []}}
        options_path = reports / (family + '-options.json')
        options_path.write_bytes(encoded(options))
        paths = {name: reports / (family + '-gate-' + name + '.json') for name in ['reference', 'off', 'capture']}
        flags = ['--discover-sysvars'] if any(row['requirement'] == 'tracked-generic-sysvar-context-v1'
                     for row in request['fixture']['runtime']['bankContext']['requiredRuntimeSysvars']) else []
        run('gate-reference-' + family, [str(capture.parent / reference['file']), str(fixture_path), str(paths['reference'])] + flags, ROOT, 300)
        run('gate-off-' + family, [str(capture), str(fixture_path), str(paths['off']), 'off'], ROOT, 300)
        run('gate-capture-' + family, [str(capture), str(fixture_path), str(paths['capture']), '--capture', str(options_path)], ROOT, 300)
        results = {name: json.loads(path.read_bytes()) for name, path in paths.items()}
        if unwrap(results['reference']) != unwrap(results['off']) or unwrap(results['off']) != unwrap(results['capture']['output']):
            raise RuntimeError('native reference/off/capture mismatch')
        observations = results['capture']
        if observations['status'] != 'COMPLETE':
            raise RuntimeError('capture did not complete')
        memory_rows = sum(len(item['memory']['rows']) for item in observations['captures'])
        if memory_rows == 0 or any(item['memory']['disposition'] != 'COMPLETE'
                                  for item in observations['captures']):
            raise RuntimeError('memory capture missing or truncated')
        modes = [item['execution_mode'] for item in observations['captures']]
        if 'execution_mode' in observations:
            modes.append(observations['execution_mode'])
        if not modes or any(value != mode for value in modes):
            raise RuntimeError('producer mode missing or differs')
        capture_pin = digest(capture.read_bytes())
        gate = {'status': 'PASS', 'family': family, 'mode': mode, 'referenceSha256': reference['sha256'],
                'captureSha256': capture_pin, 'fixtureSha256': digest(fixture_path.read_bytes()),
                'results': {name: digest(path.read_bytes()) for name, path in paths.items()}}
        (reports / (family + '-capture-gate.json')).write_bytes(encoded(gate))
        reviewed = json.loads((ROOT / 'inputs/qualified-catalog.json').read_bytes())
        original = next(row['worker'] for row in reviewed['captureWorkers'] if row['worker']['family'] == family)
        capabilities = original['capabilities']
        capabilities = list(dict.fromkeys(capabilities + ['runtime-interpreter-capture/v1']))
        source_pin = digest((ROOT / 'inputs/worker-sources.tar.gz').read_bytes())
        descriptor = dict(original, file=capture.name, sha256=capture_pin, sourceSha256=source_pin,
                          buildHash=digest(encoded({'binary': capture_pin, 'sources': source_pin, 'platform': catalog['platform']})),
                          capabilities=capabilities)
        catalog['captureWorkers'].append({'worker': descriptor, 'referenceSha256': reference['sha256'], 'gateSha256': digest(encoded(gate))})
        catalog_path.write_bytes(encoded(catalog))
        request_path = reports / (family + '-trace-request.json')
        request_path.write_bytes(encoded(request))
        policy = json.loads((fixtures / 'trace.json').read_bytes())
        original_capture_timeout = policy['capture']['limits']['timeoutMs']
        policy['capture']['limits']['timeoutMs'] = 600000
        (reports / (family + '-trace-timing.json')).write_bytes(encoded({
            'originalRequestTimeoutMs': original_request_timeout,
            'originalCaptureTimeoutMs': original_capture_timeout,
            'requestTimeoutMs': 900000, 'captureTimeoutMs': 600000,
            'historicalExpectedOutputsChanged': False,
        }))
        policy['capture']['executionMode'] = mode
        policy_path = reports / (family + '-trace-policy.json')
        policy_path.write_bytes(encoded(policy))
        # The raw gate contains large account journals. Retaining its parsed
        # trees while launching the CLI needlessly competes for runner memory.
        del results, observations, request, native, raw
        gc.collect()
        log = run('trace-' + family, [str(binary), 'simulate', '--request', str(request_path), '--trace', str(policy_path),
                  '--catalog', str(catalog_path), '--catalog-sha256', digest(encoded(catalog)), '--data-dir', str(reports / 'trace-data')], ROOT, 600)
        receipt = json.loads(log.read_bytes())
        if receipt['outcome'] != 'COMPLETED' or receipt['verification']['status'] != 'PASS' or receipt['controlVerification']['status'] != 'PASS':
            raise RuntimeError('native CLI trace failed verification')
        identity = json.loads((fixtures / ('debug/' + family + '-identity.json')).read_bytes())
        debug_row = next(row for row in matrix['debug'] if row['family'] == family)
        debug_bytes = (fixtures / debug_row['file']).read_bytes()
        if digest(debug_bytes) != debug_row['requestSha256']:
            raise RuntimeError('original debugger request digest differs')
        debug_request = rebind_request(json.loads(debug_bytes), reference)
        original_debug_timeout = debug_request['limits']['timeoutMs']
        debug_request['limits']['timeoutMs'] = 900000
        debug_path = reports / (family + '-debug-request.json')
        debug_path.write_bytes(encoded(debug_request))
        identity.update(catalogPath=str(catalog_path), catalogSha256=digest(encoded(catalog)), requestPath=str(debug_path))
        original_timeout = identity['policy']['limits']['timeoutMs']
        request_timeout = debug_request['limits']['timeoutMs']
        if original_timeout < request_timeout:
            # Qualify functionality against the existing outer request budget,
            # not a faster-host capture timing assumption. Preserve the source
            # policy and record this derived timing-only configuration explicitly.
            identity['policy']['limits']['timeoutMs'] = 600000
            (reports / (family + '-debug-timing.json')).write_bytes(encoded({
                'originalCaptureTimeoutMs': original_timeout,
                'qualificationCaptureTimeoutMs': 600000,
                'originalRequestTimeoutMs': original_debug_timeout,
                'requestTimeoutMs': request_timeout,
                'reason': 'native-host qualification uses the outer request deadline',
                'historicalExpectedOutputsChanged': False,
            }))
        identity_path = reports / (family + '-debug-identity.json')
        identity_path.write_bytes(encoded(identity))
        run('debug-' + family, ['node', str(ROOT / 'scripts/native_debug.mjs'), str(binary),
            str(reports / ('debug-' + family)), str(identity_path)], ROOT, 1000)
        if family == 'v2-3':
            for action in ['eof', 'sigterm', 'malformed']:
                run('debug-' + action, ['node', str(ROOT / 'scripts/native_debug.mjs'), str(binary),
                    str(reports / ('debug-' + action)), str(identity_path), action], ROOT, 600)
