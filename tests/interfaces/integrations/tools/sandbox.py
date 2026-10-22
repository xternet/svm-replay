"""Prove the offline macOS test guard permits local IPC, not external TCP."""
import json
import os
from pathlib import Path
import subprocess
import sys

BASE = '(version 1)(allow default)(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))'
IPC = '(allow network-outbound (remote unix-socket))'
PROBE = '''import socket,tempfile,pathlib
with tempfile.TemporaryDirectory(dir='/tmp') as root:
 path=str(pathlib.Path(root)/'ipc')
 server=socket.socket(socket.AF_UNIX);server.bind(path);server.listen()
 client=socket.socket(socket.AF_UNIX);client.connect(path)
 peer,_=server.accept();peer.close();client.close();server.close()
 print('LOCAL_IPC_OK')
'''


def main():
    if sys.platform != 'darwin':
        raise RuntimeError('Actual macOS required for the sandbox proof')
    rows = []
    for policy,success in [(BASE,False),(BASE+IPC,True)]:
        result = subprocess.run(['sandbox-exec','-p',policy,sys.executable,'-c',PROBE],
                                capture_output=True,text=True,timeout=10)
        if success:
            if result.returncode != 0 or 'LOCAL_IPC_OK' not in result.stdout:
                raise RuntimeError('Local IPC must work: '+result.stderr)
        elif result.returncode == 0 or 'PermissionError' not in result.stderr:
            raise RuntimeError('Old guard must reproduce IPC refusal: '+result.stderr)
        rows.append({'expectedSuccess':success,'exitCode':result.returncode,
                     'stdout':result.stdout,'stderr':result.stderr})
    path = Path(os.environ['TOOL_WORK'])/'reports/sandbox.json'
    path.write_text(json.dumps({'status':'PASS','redGreen':rows},indent=2)+'\n')


if __name__ == '__main__':
    main()
