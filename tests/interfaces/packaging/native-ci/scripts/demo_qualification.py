"""Final-layout archive and both installers after native historical parity passes."""
import json
import os
from pathlib import Path
import shutil
from native_matrix import ROOT, encoded, digest
from probe import command
from release_inputs import release_version
from release_archive import pack
from native_install import qualify as native_install, installer_shells
from source_integrity import verify_sources

reports = ROOT / 'reports'
summary = json.loads((reports / 'summary.json').read_bytes())
if summary['status'] != 'ALL_NATIVE_CHECKS_PASS' and not (
        summary['status'] == 'SDK_TRACE_DEBUG_PASS'
        and summary.get('previousCorpus', {}).get('coordinatorAndRequestsUnchanged') is True):
    raise RuntimeError('full native baseline must pass first')
bundle = reports / 'full-bundle'
request = reports / 'm5-case-01.json'
receipt = json.loads((reports / 'replay-m5-case-01.log').read_bytes())
if receipt['outcome'] != 'COMPLETED':
    raise RuntimeError('qualified original demo result missing')
demo = bundle / 'demo'
demo.mkdir()
shutil.copyfile(request, demo / 'request.json')
(demo / 'manifest.json').write_bytes(encoded({'schema':'svm-replay-demo/v1',
    'caseId':'m5-case-01','requestSha256':digest(request.read_bytes()),
    'expectedOutputSha256':receipt['output']['sha256']}))
manifest = json.loads((bundle / 'bundle.json').read_bytes())
for name in ['request.json', 'manifest.json']:
    data = (demo / name).read_bytes()
    manifest['files'].append({'path':'demo/'+name,'sha256':digest(data),'bytes':len(data),'executable':False})
(bundle / 'bundle.json').write_bytes(encoded(manifest))
platform = json.loads((bundle / 'catalog.json').read_bytes())['platform']
target = platform['os']+'-'+platform['arch']
release = reports / 'installer-inputs'
release.mkdir()
source = Path(os.environ['SVM_REPLAY_VALIDATION_SOURCE']).resolve(strict=True)
checks = []
work = Path(summary['workDirectory']).resolve(strict=True)
if work.is_relative_to(source):
    raise RuntimeError('Native build directory must be outside the coordinator workspace')
def integrity_run(name, args, cwd, seconds):
    log = reports / (name + '.log')
    item = command(args, cwd, log, seconds, os.environ.copy())
    checks.append(dict(item, name=name))
    if item['status'] != 'PASS':
        raise RuntimeError('Post-build source verification failed: ' + name)
    return log
summary['postBuildSourceIntegrity'] = verify_sources(integrity_run, source,
    work/'workers/worker-sources', summary['sourceCommit'],
    summary['workerSourcesSha256'], ROOT/'inputs/worker-sources.tar.gz')
result = pack(bundle, work/'workers/worker-sources',
              work/'licenses/coordinator-license-additions', source,
              release, release_version(source), platform)
raw = (release/'release.json').read_bytes()
installs = [native_install(source, release, reports, result['platforms'][0], shell)
            for shell in installer_shells(os.name)]
(reports/'native-install-summary.json').write_bytes(encoded({'status': 'PASS', 'installations': installs}))
entry = result['platforms'][0]
(release/'candidate-proof.json').write_bytes(encoded({'sourceCommit': summary['sourceCommit'],
    'platform': entry['platform'], 'archiveSha256': entry['sha256'],
    'bundleSha256': entry['bundleSha256'], 'sdkTarballSha256': summary['sdkTarballSha256'],
    'postBuildSourceIntegrity': summary['postBuildSourceIntegrity']}))
def run(name, args, seconds=300):
    item = command(args, ROOT, reports/(name+'.log'), seconds, os.environ.copy())
    checks.append(dict(item, name=name))
    if item['status'] != 'PASS':
        print((reports/(name+'.log')).read_text(errors='replace')[-10000:], flush=True)
        raise RuntimeError(name+' failed')
    return json.loads((reports/(name+'.log')).read_bytes())
installed = reports / 'installed-demo'
run('guided-install', ['node', str(source/'src/bindings/typescript/install.mjs'),
    '--release',str(release/'release.json'),'--sha256',digest(raw),'--output',str(installed)])
binary = installed / 'bin' / ('svm-replay.exe' if os.name=='nt' else 'svm-replay')
receipts = set()
for iteration in range(2):
    result = run('cached-demo-'+str(iteration), [str(binary),'demo','--data-dir',str(reports/'demo-data')])
    if result['outcome']!='COMPLETED' or result['matched'] is not True or 'cache' in result['receipt']:
        raise RuntimeError('demo did not freshly match without cache')
    if result['receiptPath'] in receipts or not result['receipt']['phases']:
        raise RuntimeError('demo reused a receipt or skipped execution')
    if any(phase['process']['pid'] <= 0 for phase in result['receipt']['phases']):
        raise RuntimeError('demo phase missing fresh worker process')
    receipts.add(result['receiptPath'])
    if result['receipt']['output']['sha256'] != receipt['output']['sha256']:
        raise RuntimeError('demo changed historical output')
(reports/'demo-summary.json').write_bytes(encoded({'status':'PASS','checks':checks,
    'caseId':'m5-case-01','networkAcquisition':False,'redistributionApproved':False}))
