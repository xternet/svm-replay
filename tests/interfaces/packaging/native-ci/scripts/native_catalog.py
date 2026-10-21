"""Keep reviewed runtime identity separate from portable build provenance."""
import hashlib
import json


def reference_descriptor(build, reviewed, registry, file, worker_pin, build_pin):
    inputs = (json.dumps(build['inputFiles'], sort_keys=True, separators=(',', ':'),
                         ensure_ascii=False) + '\n').encode()
    if hashlib.sha256(inputs).hexdigest() != build['portableInputSha256']:
        raise ValueError('portable worker input inventory differs')
    sources = [row for row in registry['executors']
               if row['id'] == reviewed['executorSourceId']]
    if (len(sources) != 1 or build['family'] != reviewed['family']
            or sources[0]['packageName'] != reviewed['family']
            or sources[0]['reviewedSourceSha256'] != reviewed['sourceSha256']
            or build['retainedSourceSha256'] != reviewed['sourceSha256']):
        raise ValueError('worker source differs from the reviewed runtime profile')
    # sourceSha256 identifies reviewed VM code, not path labels in a portable
    # source packet. The build hash separately binds that entire pinned packet.
    return dict(reviewed, file=file, sha256=worker_pin, buildHash=build_pin)
