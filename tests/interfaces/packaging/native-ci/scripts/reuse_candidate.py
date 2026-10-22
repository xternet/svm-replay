"""Check changed coordinator/installers against retained, hash-bound native workers."""
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import copy
from pathlib import Path
from build_paths import build_environment, create_work
from fresh_consumer import verify_download
from native_host import require_binary_arch
from probe import command, safe_members
from release_archive import digest, pack
from release_inputs import release_version
from reuse_provenance import verify_bundle, verify_source_reuse
from source_integrity import verify_workers


def main() -> None:
    source = Path.cwd().resolve()
    root = source / "tests/interfaces/packaging/native-ci"
    reports = root / "reports"
    provenance = json.loads((reports / "reuse-producer-summary.json").read_bytes())
    system = {"darwin": "macos", "win32": "windows"}[sys.platform]
    arch = {"x86_64": "x86_64", "AMD64": "x86_64", "arm64": "aarch64",
            "ARM64": "aarch64"}[platform.machine()]
    work = create_work(source)
    env = build_environment(source, work)
    summary = {"status": "IN_PROGRESS", "runtimeBuilds": 0, "producer": provenance,
               "checks": [], "platform": system + "-" + arch}

    def save():
        (reports / "reuse-summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    def run(name, argv, cwd=source, seconds=600, child_env=None):
        log = reports / (name + ".log")
        result = command(argv, cwd, log, seconds, env if child_env is None else child_env)
        summary["checks"].append(dict(result, name=name))
        save()
        print(json.dumps(dict(result, name=name)), flush=True)
        if result["status"] != "PASS":
            print(log.read_text(errors="replace")[-12000:], flush=True)
            raise RuntimeError("Retained-artifact check failed: " + name)
        return log

    try:
        actual = run("reuse-source", ["git", "rev-parse", "HEAD"]).read_text().strip()
        if actual != os.environ["GITHUB_SHA"] or actual != provenance["validationCommit"]:
            raise ValueError("Reuse validation source differs")
        run("reuse-source-clean", ["git", "diff", "--exit-code", "HEAD", "--"])
        verify_source_reuse(source, provenance["sourceCommit"], provenance["refreshCoordinator"])
        entry, _ = verify_download(root / "downloads", provenance["sourceCommit"], summary["platform"])
        original_proof = json.loads((root / "downloads/installer-inputs/candidate-proof.json").read_bytes())
        archive = root / "downloads/installer-inputs" / entry["file"]
        with tarfile.open(archive) as stream:
            stream.extractall(work / "retained", members=safe_members(stream), filter="data")
        retained = work / "retained"
        bundle = retained / "bundle"
        if digest(bundle / "bundle.json") != entry["bundleSha256"]:
            raise ValueError("Retained bundle manifest differs")
        manifest = verify_bundle(bundle)
        catalog = json.loads((bundle / "catalog.json").read_bytes())
        if (len(catalog["workers"]) != 7 or len(catalog["captureWorkers"]) != 7
                or catalog["platform"] != {"os": system, "arch": arch}):
            raise ValueError("Retained runtime platform or seven-plus-seven census differs")
        workers = copy.deepcopy([row for row in manifest["files"] if row["path"].startswith("workers/")])
        if len(workers) != 14:
            raise ValueError("Retained runtime file census differs")
        old_sources = original_proof["postBuildSourceIntegrity"]
        inputs = json.loads((root / "public-inputs.json").read_bytes())
        source_pin = next(row["sha256"] for row in inputs["assets"] if row["file"] == "worker-sources.tar.gz")
        if (old_sources["workerSourcesSha256"] != source_pin
                or verify_workers(retained / "runtime-sources") != old_sources["workerSourceFiles"]):
            raise ValueError("Retained worker source inventory differs")
        binary = bundle / manifest["binary"]
        original_binary = digest(binary)
        require_binary_arch(binary, system, arch)
        if provenance["refreshCoordinator"]:
            run("reuse-rust-host", ["rustc", "+1.94.0", "-vV"])
            run("reuse-cli-build", ["cargo", "+1.94.0", "build", "--release", "--locked",
                "--jobs", "4", "-p", "svm-replay", "--bin", "svm-replay"], seconds=1800)
            built = work / "target/release" / binary.name
            require_binary_arch(built, system, arch)
            retained_cli = reports / "coordinator"
            retained_cli.mkdir()
            shutil.copy2(built, retained_cli / binary.name)
            summary["coordinatorBuild"] = {"sourceCommit": actual, "sha256": digest(built)}
            save()
            shutil.copy2(built, binary)
            row = next(row for row in manifest["files"] if row["path"] == manifest["binary"])
            row.update(sha256=digest(binary), bytes=binary.stat().st_size)
            (bundle / "bundle.json").write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")))
            run("reuse-workspace", ["cargo", "+1.94.0", "test", "--workspace", "--locked", "--jobs", "4"])
        env["SVM_REPLAY_BOUNDARY_BINARY"] = str(binary)
        npm = shutil.which("npm")
        if npm is None:
            raise RuntimeError("Node/npm unavailable")
        sdk = source / "src/bindings/typescript"
        run("reuse-sdk-dependencies", [npm, "ci", "--ignore-scripts", "--no-audit", "--no-fund"], sdk)
        run("reuse-sdk-tests", [npm, "test"], sdk)
        updated = reports / "updated"
        release, packages = updated / "installer-inputs", updated / "sdk-packed"
        release.mkdir(parents=True)
        packages.mkdir()
        run("reuse-sdk-pack", [npm, "pack", "--pack-destination", str(packages)], sdk)
        sdk_path = packages / ("xternet-svm-replay-" + release_version(source) + ".tgz")
        if provenance["refreshCoordinator"]:
            run("reuse-session-inputs", [sys.executable, str(Path(__file__).with_name("download_inputs.py"))])
            run("reuse-session", [sys.executable, str(Path(__file__).with_name("native_sessions.py")),
                "--inputs", str(root / "inputs/session"), "--binary", str(binary),
                "--catalog", str(bundle / "catalog.json"), "--source", str(source),
                "--target", str(work / "target"), "--reports", str(reports / "native-sessions")],
                seconds=1200, child_env=os.environ.copy())
        verify_bundle(bundle)
        if workers != [row for row in manifest["files"] if row["path"].startswith("workers/")]:
            raise ValueError("Retained workers changed during coordinator qualification")
        result = pack(bundle, retained / "runtime-sources", retained / "coordinator-license-additions",
                      source, release, release_version(source), catalog["platform"])
        output_entry = result["platforms"][0]
        components = {"workers": {"producerCommit": provenance["sourceCommit"],
            "producerRun": provenance["sourceRun"], "files": workers},
            "coordinator": {"producerCommit": actual if provenance["refreshCoordinator"] else provenance["sourceCommit"],
                "sha256": digest(binary), "originalSha256": original_binary}, "sdkCommit": actual}
        proof = {"sourceCommit": actual, "platform": summary["platform"],
            "archiveSha256": output_entry["sha256"], "bundleSha256": output_entry["bundleSha256"],
            "sdkTarballSha256": digest(sdk_path), "componentProvenance": components,
            "postBuildSourceIntegrity": dict(old_sources, sourceCommit=actual)}
        (release / "candidate-proof.json").write_text(json.dumps(proof, sort_keys=True) + "\n")
        summary["components"] = components
        run("reuse-guided-install", ["node", str(sdk / "install.mjs"), "--release", str(release / "release.json"),
            "--sha256", digest(release / "release.json"), "--output", str(work / "guided-install")])
        run("reuse-fresh-consumer", [sys.executable, str(Path(__file__).with_name("fresh_consumer.py")),
            str(source), str(updated), str(reports)])
        run("reuse-final-source-clean", ["git", "diff", "--exit-code", "HEAD", "--"])
        summary.update(status="PASS", qualification="retained-worker-evidence-plus-affected-checks",
                       nativeMatrixRebuilt=False, archiveSha256=output_entry["sha256"], sdkSha256=digest(sdk_path))
    except Exception as error:
        summary.update(status="FAIL", error=str(error))
        raise
    finally:
        save()


if __name__ == "__main__":
    main()
