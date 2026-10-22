"""Run rendered UI tests in an actual disposable hosted WSL2 VM, not Linux emulation."""
import os
from pathlib import Path
import subprocess


def main():
    if os.name != 'nt' or os.environ.get('GITHUB_REPOSITORY') != 'xternet/svm-replay':
        raise RuntimeError('Disposable Windows CI only')
    wsl = Path(os.environ['SystemRoot']) / 'System32/wsl.exe'
    def run(args, **kwargs):
        return subprocess.run([str(v) for v in args], check=True, timeout=1800, **kwargs)
    run([wsl, '--install', '--distribution', 'Ubuntu-24.04', '--no-launch', '--web-download'])
    guest = [wsl, '-d', 'Ubuntu-24.04', '-u', 'root', '--exec']
    kernel = run([*guest, 'uname', '-r'], capture_output=True).stdout.decode().strip()
    if 'microsoft-standard-wsl2' not in kernel.lower():
        raise RuntimeError('Actual WSL2 kernel required')
    def path(value):
        return run([*guest, 'wslpath', '-a', '-u', str(value)], capture_output=True).stdout.decode().strip()
    reports = Path(os.environ['RUNNER_TEMP']) / 'wsl-tool-reports'
    reports.mkdir()
    owner = Path(__file__).resolve().parent
    script = owner.joinpath('wsl.sh').read_bytes().replace(b'\r\n', b'\n')
    run([*guest, 'bash', '-s', '--', path(owner), path(Path(os.environ['RUNNER_TEMP']) / 'native'),
         path(reports), kernel], input=script)


if __name__ == '__main__':
    main()
