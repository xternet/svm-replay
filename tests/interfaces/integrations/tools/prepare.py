"""Install pinned qualification inputs; never acquire additional chain history."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fetch(url, expected, output):
    with urllib.request.urlopen(url, timeout=180) as response, output.open('xb') as stream:
        shutil.copyfileobj(response, stream)
    if digest(output) != expected:
        raise ValueError('Qualification input checksum mismatch: ' + output.name)


def extract(path, destination):
    destination.mkdir(parents=True)
    with tarfile.open(path) as archive:
        for member in archive.getmembers():
            if not (member.isfile() or member.isdir()):
                raise ValueError('Only plain archive entries accepted')
        archive.extractall(destination, filter='data')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('platform', choices=['macos-aarch64', 'macos-x86_64', 'linux-x86_64'])
    parser.add_argument('native', type=Path)
    parser.add_argument('work', type=Path)
    args = parser.parse_args()
    work = args.work.resolve()
    work.mkdir(parents=True)
    pins = json.loads(Path(__file__).with_name('inputs.json').read_bytes())
    entry = pins['native'][args.platform]
    archives = list(args.native.rglob(entry['name']))
    if len(archives) != 1 or digest(archives[0]) != entry['sha256']:
        raise ValueError('Exactly the selected qualified native archive is required')
    extract(archives[0], work / 'native')
    bundle = work / 'native/bundle'
    subprocess.run([str(bundle / 'bin/svm-replay'), 'bundle', 'install', '--bundle',
                    str(bundle / 'bundle.json'), '--bundle-sha256', digest(bundle / 'bundle.json'),
                    '--output', str(work / 'installed')], check=True, timeout=120)
    subprocess.run([str(work / 'installed/bin/svm-replay'), 'doctor'], check=True, timeout=60)
    base = 'https://github.com/xternet/svm-replay/releases/download/validation-inputs-0.2.0/'
    for name, item in pins['inputs'].items():
        fetch(base + item['name'], item['sha256'], work / name)
    extract(work / 'fixture.tar.gz', work / 'fixture')
    extract(work / 'toy.tar.gz', work / 'toy')
    workspace = work / 'toy/workspace'
    for path in (work / 'fixture/svm-replay-smoke-413023260').glob('files.json'):
        for row in json.loads(path.read_bytes()):
            item = path.parent / row['path']
            if not item.resolve().is_relative_to(path.parent.resolve()):
                raise ValueError('Fixture manifest escapes its root')
            if digest(item) != row['sha256']:
                raise ValueError('Fixture manifest checksum mismatch')
    anchor = os.environ.get('ANCHOR_SOURCE')
    if anchor:
        (workspace / '.upstream-anchor').symlink_to(Path(anchor).resolve(), target_is_directory=True)
    subprocess.run(['npm', 'ci', '--ignore-scripts', '--no-audit', '--no-fund'],
                   cwd=workspace, check=True, timeout=300)
    (work / 'reports').mkdir()
    print(json.dumps({'status': 'PREPARED', 'platform': args.platform,
                      'nativeSha256': entry['sha256']}))


if __name__ == '__main__':
    main()
