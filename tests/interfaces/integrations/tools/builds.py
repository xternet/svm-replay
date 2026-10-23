"""Retain only the three exact tool executables needed for test-only reruns."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys

SOURCES = {'surfpool':'33262045b129d2c722178b8436035cc957d44bee',
           'anchor':'288cca71a754556debde34b7c465092dbe5f84da'}


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()


def main():
    mode, directory = sys.argv[1:]
    root = Path(directory)
    if mode == 'save':
        root.mkdir()
        records = [json.loads(line) for line in
                   Path(os.environ['TOOL_WORK'],'sdk-build.jsonl').read_text().splitlines()]
        sdk = [r['executable'] for r in records if
               r.get('target',{}).get('name')=='historical' and r.get('executable')]
        if len(sdk) != 1:
            raise ValueError('Exactly one actual historical SDK test executable required')
        files = {'surfpool':Path(os.environ['CARGO_TARGET_DIR'])/'debug/surfpool',
                 'anchor':Path(os.environ['CARGO_TARGET_DIR'])/'debug/anchor',
                 'historical':Path(sdk[0])}
        for name,path in files.items():
            shutil.copyfile(path,root/name)
            (root/name).chmod(0o755)
        (root/'build.json').write_text(json.dumps({'sources':SOURCES,
            'platform':os.environ['TOOL_PLATFORM'],
            'files':{name:digest(root/name) for name in files}},indent=2)+'\n')
    elif mode != 'verify':
        raise ValueError('save or verify required')
    info = json.loads((root/'build.json').read_bytes())
    if info['sources'] != SOURCES or info['platform'] != os.environ['TOOL_PLATFORM']:
        raise ValueError('Retained tool source/platform mismatch')
    if set(info['files']) != {'surfpool','anchor','historical'}:
        raise ValueError('Retained executable inventory mismatch')
    for name,expected in info['files'].items():
        if digest(root/name) != expected:
            raise ValueError('Retained tool hash mismatch: '+name)
        (root/name).chmod(0o755)
    print(json.dumps({'status':'VERIFIED','build':info}))


if __name__ == '__main__':
    main()
