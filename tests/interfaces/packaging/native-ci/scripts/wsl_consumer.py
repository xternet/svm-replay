"""Consume qualified Linux bytes in a real WSL2 VM on an ephemeral hosted runner."""
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from fresh_consumer import verify_download


def require_commit(commit: str) -> str:
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise ValueError("WSL candidate source commit is malformed")
    return commit


def require_wsl2(kernel: str) -> None:
    if "microsoft-standard-wsl2" not in kernel.lower():
        raise ValueError("Actual WSL2 kernel required; ordinary Linux/WSL1 is not qualification")


def main() -> None:
    if (sys.platform != "win32" or os.environ.get("GITHUB_ACTIONS") != "true"
            or os.environ.get("GITHUB_REPOSITORY") != "xternet/svm-replay"):
        raise ValueError("This WSL installer is restricted to the disposable hosted CI job")
    source = require_commit(os.environ["GITHUB_SHA"])
    mode = os.environ["SVM_REPLAY_WSL_PROBE"]
    if mode not in ("true", "false"):
        raise ValueError("Explicit WSL probe/consumer mode is required")
    probe = mode == "true"
    owner = Path(__file__).resolve().parent
    reports = owner.parent / "reports/wsl"
    reports.mkdir(parents=True)
    summary = {"status": "IN_PROGRESS", "run": os.environ["GITHUB_RUN_ID"],
               "runtimeBuilds": 0, "probeOnly": probe, "sourceCommit": source}

    def command(argv, capture=False, **kwargs):
        print("Running:", json.dumps([str(arg) for arg in argv]), flush=True)
        return subprocess.run([str(arg) for arg in argv], check=True, timeout=1200,
                              capture_output=capture, **kwargs)

    try:
        downloads = Path(os.environ["RUNNER_TEMP"]) / "svm-native-download"
        if not probe:
            entry, _ = verify_download(downloads, source, "linux-x86_64")
            summary["archiveSha256"] = entry["sha256"]
        wsl = Path(os.environ["SystemRoot"]) / "System32/wsl.exe"
        distro = [wsl, "--distribution", "Ubuntu-24.04", "--user", "root", "--"]
        command([wsl, "--install", "--distribution", "Ubuntu-24.04", "--no-launch", "--web-download"])
        kernel = command([*distro, "uname", "-r"], capture=True).stdout.decode("utf-8").strip()
        require_wsl2(kernel)
        (reports / "wsl-kernel.txt").write_text(kernel + "\n")
        summary["kernel"] = kernel
        if probe:
            summary.update(status="WSL2_AVAILABLE_NOT_QUALIFIED", scope="WSL2 setup/kernel only; no SVM Replay execution")
            return

        def guest_path(path):
            return command([*distro, "wslpath", "-a", "-u", path.resolve()],
                           capture=True).stdout.decode("utf-8").strip()

        script = owner.joinpath("wsl_guest.sh").read_bytes().replace(b"\r\n", b"\n")
        command([*distro, "bash", "-s", "--", source, guest_path(downloads), guest_path(reports)],
                input=script)
        result = json.loads((reports / "fresh-consumer-summary.json").read_bytes())
        if result["status"] != "PASS" or result["sourceCommit"] != source:
            raise ValueError("WSL consumer did not pass on the candidate source")
        summary.update(status="PASS", scope="WSL2 install, fourteen-worker doctor, demo and packed SDK replay",
                       providerCalls=False, compilerRequired=False)
    except Exception as error:
        summary.update(status="FAIL", error=str(error))
        raise
    finally:
        (reports / "wsl-summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
