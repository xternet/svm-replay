"""Install downloaded candidate bytes on a new runner; never build a runtime."""
import json
import os
import platform
import shutil
import sys
from pathlib import Path
from native_host import require_binary_arch
from native_install import installer_shells, qualify
from probe import command
from release_archive import digest


def verify_download(downloads: Path, commit: str, target: str) -> tuple[dict, Path]:
    release = downloads / "installer-inputs"
    manifest = json.loads((release / "release.json").read_bytes())
    proof = json.loads((release / "candidate-proof.json").read_bytes())
    if len(manifest["platforms"]) != 1:
        raise ValueError("Candidate platform count differs")
    entry = manifest["platforms"][0]
    if entry["platform"] != target or Path(entry["file"]).name != entry["file"]:
        raise ValueError("Downloaded platform or archive owner differs")
    archives = list((downloads / "sdk-packed").glob("*.tgz"))
    if len(archives) != 1:
        raise ValueError("Downloaded SDK package count differs")
    sdk = archives[0]
    if (proof["sourceCommit"] != commit or proof["platform"] != target
            or proof["archiveSha256"] != entry["sha256"]
            or digest(release / entry["file"]) != entry["sha256"]
            or proof["bundleSha256"] != entry["bundleSha256"]
            or proof["sdkTarballSha256"] != digest(sdk)):
        raise ValueError("Downloaded candidate identity differs")
    return entry, sdk


def main() -> None:
    source, downloads, reports = (Path(arg).resolve(strict=True) for arg in sys.argv[1:4])
    operating_system = {"linux": "linux", "darwin": "macos", "win32": "windows"}[sys.platform]
    architecture = {"x86_64": "x86_64", "AMD64": "x86_64", "aarch64": "aarch64",
                    "arm64": "aarch64", "ARM64": "aarch64"}[platform.machine()]
    expected_arch = {"x86_64": "X64", "aarch64": "ARM64"}[architecture]
    if os.environ["RUNNER_ARCH"].upper() != expected_arch:
        raise ValueError("Fresh-runner architecture differs")
    entry, sdk = verify_download(downloads, os.environ["GITHUB_SHA"], operating_system + "-" + architecture)
    summary = {"status": "IN_PROGRESS", "sourceCommit": os.environ["GITHUB_SHA"],
               "archiveSha256": entry["sha256"], "sdkSha256": digest(sdk), "checks": []}

    def run(name, argv):
        log = reports / (name + ".log")
        result = command(argv, reports, log, 600, os.environ.copy())
        summary["checks"].append(dict(result, name=name))
        if result["status"] != "PASS":
            raise RuntimeError("Fresh consumer failed: " + name + "; " + str(log))
        return log.read_text().strip()

    try:
        actual = run("source-commit", ["git", "-C", str(source), "rev-parse", "HEAD"])
        if actual != os.environ["GITHUB_SHA"]:
            raise ValueError("Consumer source checkout differs")
        run("source-checkout", ["git", "-C", str(source), "diff", "--exit-code", "HEAD", "--"])
        installs = [qualify(source, downloads / "installer-inputs", reports, entry, shell)
                    for shell in installer_shells(os.name)]
        prefix = Path(installs[0]["prefix"])
        if digest(prefix / "bundle.json") != entry["bundleSha256"]:
            raise ValueError("Installed bundle differs from the tested candidate")
        binary = prefix / "bin" / ("svm-replay.exe" if os.name == "nt" else "svm-replay")
        require_binary_arch(binary, operating_system, architecture)
        consumer = reports / "sdk consumer spaces"
        consumer.mkdir()
        (consumer / "package.json").write_text('{"private":true,"type":"module"}\n')
        npm = shutil.which("npm")
        if npm is None:
            raise RuntimeError("Node/npm unavailable on consumer runner")
        run("fresh-sdk-install", [npm, "install", "--prefix", str(consumer),
            "--ignore-scripts", "--no-audit", "--no-fund", str(sdk)])
        run("fresh-sdk-execution", ["node", str(Path(__file__).with_name("fresh_sdk.mjs")),
            str(consumer / "node_modules/@xternet/svm-replay/dist/index.js"), str(prefix), str(reports)])
        summary.update(status="PASS", installations=installs, runtimeBuilds=0,
                       compilerRequired=False, globalActivation=False)
    except Exception as error:
        summary.update(status="FAIL", error=str(error))
        raise
    finally:
        (reports / "fresh-consumer-summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
