"""Run native bootstrap, launcher, doctor and fresh demo on the exact candidate archive."""
import json
import os
from pathlib import Path
from probe import command
from release_archive import digest


def installer_shells(system: str) -> list[str]:
    if system == "nt":
        return ["pwsh", "powershell.exe"]
    if system == "posix":
        return ["sh"]
    raise ValueError("Unqualified installer operating system")


def qualify(source: Path, release: Path, reports: Path, entry: dict,
            shell: str | None = None) -> dict:
    shell = installer_shells(os.name)[0] if shell is None else shell
    if shell not in installer_shells(os.name):
        raise ValueError("Unqualified installer shell")
    archive = release / entry["file"]
    if digest(archive) != entry["sha256"]:
        raise ValueError("Native installation archive differs")
    label = shell.replace(".exe", "")
    common = reports / ("native install spaces " + label)
    common.mkdir()
    prefix, bin_dir = common / "installation", common / "commands"
    if os.name == "nt":
        args = [shell, "-NoLogo", "-NoProfile", "-File", str(source / "install.ps1"),
                "-Archive", str(archive), "-Sha256", entry["sha256"],
                "-Prefix", str(prefix), "-BinDir", str(bin_dir)]
        launcher = bin_dir / "svm-replay.cmd"
    else:
        args = ["sh", str(source / "install.sh"), "--archive", str(archive),
                "--sha256", entry["sha256"], "--prefix", str(prefix), "--bin-dir", str(bin_dir)]
        launcher = bin_dir / "svm-replay"
    checks = []

    def run(name, argv):
        log = reports / (label + "-" + name + ".log")
        result = command(argv, reports, log, 600, os.environ.copy())
        checks.append(dict(result, name=name))
        if result["status"] != "PASS":
            raise RuntimeError(f"Native installer check failed: {name}; {log}")
        return log.read_text()

    run("native-bootstrap", args)
    if not launcher.is_file():
        raise RuntimeError("Native bootstrap did not publish its launcher")

    def launch(argv):
        if os.name == "nt":
            return [shell, "-NoLogo", "-NoProfile", "-File",
                    str(Path(__file__).with_name("run_native_launcher.ps1")),
                    "-Launcher", str(launcher), *argv]
        return [str(launcher), *argv]

    doctor = json.loads(run("native-bootstrap-doctor", launch(["doctor", "--json"])))
    if doctor["outcome"] != "COMPLETED" or len(doctor["workers"]) != 7 or len(doctor["captureWorkers"]) != 7:
        raise RuntimeError("Native doctor did not verify all fourteen workers")
    demo = json.loads(run("native-bootstrap-demo", launch(["demo", "--json", "--example", "default",
                                                          "--data-dir", str(common / "demo-data")])))
    if demo["outcome"] != "COMPLETED" or demo["matched"] is not True:
        raise RuntimeError("Native bootstrap demo did not freshly match")
    for name in ("runtime-sources", "coordinator-license-additions"):
        if not any(path.is_file() for path in (prefix / name).rglob("*")):
            raise RuntimeError("Native install lost source/license payloads")
    result = {"schema": "native-bootstrap-qualification/v1", "status": "PASS", "checks": checks,
              "archiveSha256": entry["sha256"], "actualPlatform": os.name,
              "shell": shell, "prefix": str(prefix), "globalActivation": False}
    (reports / (label + "-native-install-summary.json")).write_text(json.dumps(result, indent=2) + "\n")
    return result
