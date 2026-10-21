"""Extend immutable 47-case fixtures with an independently qualified early case."""
import hashlib
import copy
import json
import shutil
import tomllib
from pathlib import Path


def rebind_request(original, worker):
    """Replace build identity only; never mutate historical inputs or expected results."""
    request = copy.deepcopy(original)
    binding = request["fixture"]["runtime"]["binding"]
    executor = binding["executor"]
    if executor["id"] != worker["executorSourceId"]:
        raise ValueError("native worker executor differs from the historical request")
    executor["sourceEvidenceHash"] = worker["sourceSha256"]
    executor["m9Build"]["capabilities"] = list(worker["capabilities"])
    executor["m9Build"]["binarySha256"] = worker["sha256"]
    executor["m9Build"]["buildHash"] = worker["buildHash"]
    del binding["bindingHash"]
    canonical = json.dumps(binding, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    binding["bindingHash"] = hashlib.sha256(b"m6-runtime-binding/v1\n" + canonical + b"\n").hexdigest()
    return request


def release_version(source: Path) -> str:
    version = tomllib.loads((source / "Cargo.toml").read_text())["package"]["version"]
    sdk = json.loads((source / "src/bindings/typescript/package.json").read_bytes())
    if not isinstance(version, str) or not version:
        raise ValueError("invalid CLI package version")
    if sdk["version"] != version:
        raise ValueError("CLI and SDK release versions differ")
    return version


def unwrap(value):
    if value.get('schema') != 'svm-sysvar-discovery/v1':
        return value
    if value['status'] != 'COMPLETE':
        raise ValueError('incomplete sysvar discovery is not an execution result')
    return value['output']


def require_families(cases, families):
    if {row['family'] for row in cases} != set(families):
        raise ValueError('runtime coverage differs from the declared build matrix')


def add_early(root, fixtures, matrix):
    row = json.loads((root / 'inputs/early-case.json').read_bytes())
    raw = (root / 'inputs/early-request.json').read_bytes()
    if hashlib.sha256(raw).hexdigest() != row['requestSha256']:
        raise ValueError('early historical input pin differs')
    if row['family'] != 'v2-2' or any(r['id'] == row['id'] for r in matrix['cases']):
        raise ValueError('early case collision or family differs')
    debug = json.loads((root / 'inputs/early-debug.json').read_bytes())
    if any(r['family'] == row['family'] for r in matrix['debug']):
        raise ValueError('early debugger family collision')
    shutil.copyfile(root / 'inputs/early-request.json', fixtures / row['file'])
    matrix['cases'].append(row)
    matrix['debug'].append({'family':row['family'],'file':row['file'],
                            'requestSha256':row['requestSha256']})
    (fixtures / 'debug').mkdir(exist_ok=True)
    (fixtures / 'debug/v2-2-identity.json').write_text(json.dumps(debug))
    (fixtures / 'cases.json').write_text(json.dumps(matrix))
    traces_path = fixtures / 'traces.json'
    traces = json.loads(traces_path.read_bytes())
    traces.append({'caseId':row['id'], 'file':row['file'], 'sha256':row['requestSha256']})
    traces_path.write_text(json.dumps(traces))
