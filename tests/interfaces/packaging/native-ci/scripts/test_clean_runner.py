import hashlib
import inspect
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
import native_install
import native_matrix


class CleanRunnerTests(unittest.TestCase):
    def test_generated_ci_roots_are_ignored_but_the_actual_harness_is_not(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        generated = [f"tests/interfaces/packaging/native-ci/{name}/probe.json"
                     for name in ("inputs", "work", "reports", "downloads")]
        source = "tests/interfaces/packaging/native-ci/scripts/native_matrix.py"
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(["git", "init", "--quiet", directory], check=True)
            Path(directory, ".gitignore").write_bytes((root / ".gitignore").read_bytes())
            ignored = subprocess.check_output(["git", "-c", "core.excludesFile=/dev/null",
                "check-ignore", "--no-index", "-z", "--stdin"], cwd=directory,
                input=b"\0".join(path.encode() for path in [*generated, source]) + b"\0")
            self.assertEqual(ignored.split(b"\0"), [*(path.encode() for path in generated), b""])

    def test_offline_package_checks_run_after_sdk_dependencies_are_installed(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        job = workflow.split("  offline-checks:\n", 1)[1].split("  native-candidate:\n", 1)[0]
        self.assertLess(job.index("      - name: TypeScript binding and package"),
                        job.index("      - name: Package and contributor safeguards"))

    def test_ordinary_sdk_job_builds_its_native_boundary_prerequisite(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        job = workflow.split("  portable-sdk:\n", 1)[1].split("  offline-checks:\n", 1)[0]
        self.assertIn("SVM_REPLAY_BOUNDARY_BINARY:", job)
        job_env = job.split("    env:\n", 1)[1].split("    steps:\n", 1)[0]
        self.assertNotIn("runner.", job_env)
        self.assertIn("contains(matrix.os, 'windows-')", job_env)
        self.assertIn("cargo +1.94.0 build --locked --jobs 2 -p svm-replay --bin svm-replay", job)
        self.assertLess(job.index("cargo +1.94.0 build"), job.index("      - name: SDK build and tests"))

    def test_python_harness_failure_cannot_be_masked_by_a_later_command(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        check = workflow.split("      - name: Qualification harness checks\n", 1)[1]
        check = check.split("      - name:", 1)[0]
        self.assertIn("python -m unittest", check)
        self.assertNotIn("node --test", check)
        self.assertIn("      - name: Process identity harness checks\n", workflow)

    def test_clean_consumer_runs_on_a_separate_runner_after_native_builds(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        consumer = workflow.split("  clean-consumer:\n", 1)[1]
        self.assertIn("needs: native-candidate", consumer)
        self.assertIn("actions/download-artifact@", consumer)
        self.assertIn("fresh_consumer.py", consumer)
        self.assertNotIn("cargo ", consumer)
        self.assertNotIn("rustup ", consumer)

    def test_windows_qualifies_both_powershell_versions(self):
        self.assertEqual(native_install.installer_shells("nt"), ["pwsh", "powershell.exe"])
        self.assertEqual(native_install.installer_shells("posix"), ["sh"])
        with self.assertRaises(ValueError):
            native_install.installer_shells("unknown")

    def test_worker_source_recheck_rejects_changed_or_missing_source(self):
        from source_integrity import verify_workers
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "worker.rs"
            source.write_text("retained source")
            manifest = {"files": [{"path": source.name,
                "sha256": hashlib.sha256(source.read_bytes()).hexdigest()}]}
            (root / "release.json").write_text(json.dumps(manifest))
            self.assertEqual(verify_workers(root), 1)
            source.write_text("changed source")
            with self.assertRaisesRegex(ValueError, "source differs"):
                verify_workers(root)
            source.unlink()
            with self.assertRaises(FileNotFoundError):
                verify_workers(root)

    def test_worker_source_path_cannot_escape(self):
        from source_integrity import verify_workers
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "release.json").write_text(json.dumps({"files": [
                {"path": "../outside.rs", "sha256": "0" * 64}]}))
            with self.assertRaisesRegex(ValueError, "escapes"):
                verify_workers(root)

    def test_archive_checks_post_build_sources_before_pack(self):
        source = Path(native_matrix.__file__).with_name("demo_qualification.py").read_text()
        self.assertIn("verify_sources(", source)
        self.assertLess(source.index("verify_sources("), source.index("result = pack("))

    def test_downloaded_artifact_consumer_checks_exact_source_sdk_and_archive(self):
        from fresh_consumer import verify_download
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            release = root / "installer-inputs"
            release.mkdir()
            sdk_dir = root / "sdk-packed"
            sdk_dir.mkdir()
            archive, sdk = release / "candidate.tar.gz", sdk_dir / "sdk.tgz"
            archive.write_bytes(b"archive")
            sdk.write_bytes(b"sdk")
            digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
            entry = {"platform": "linux-x86_64", "file": archive.name,
                "sha256": digest(archive), "bundleSha256": "a" * 64}
            (release / "release.json").write_text(json.dumps({"platforms": [entry]}))
            proof = {"sourceCommit": "b" * 40, "platform": entry["platform"],
                "archiveSha256": entry["sha256"], "bundleSha256": entry["bundleSha256"],
                "sdkTarballSha256": digest(sdk)}
            (release / "candidate-proof.json").write_text(json.dumps(proof))
            self.assertEqual(verify_download(root, "b" * 40, "linux-x86_64")[1], sdk)
            for key in ("sourceCommit", "platform", "archiveSha256", "bundleSha256", "sdkTarballSha256"):
                bad = dict(proof, **{key: "wrong"})
                (release / "candidate-proof.json").write_text(json.dumps(bad))
                with self.subTest(key=key), self.assertRaisesRegex(ValueError, "differs"):
                    verify_download(root, "b" * 40, "linux-x86_64")


if __name__ == "__main__":
    unittest.main()
