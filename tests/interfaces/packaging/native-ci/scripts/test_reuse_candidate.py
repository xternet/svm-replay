"""Reuse must retain the producer identity and reject changed runtime inputs."""
import copy
import hashlib
import json
import os
import subprocess
import tempfile
import unittest
import ast
from pathlib import Path
from reuse_provenance import require_producer, verify_bundle, verify_source_reuse


class ReuseTests(unittest.TestCase):
    def test_sdk_checkout_has_identical_text_on_windows_and_posix(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        names = ["README.md", "install.mjs", "installation.mjs", "launcher.mjs", "fork.mjs", "package.json"]
        paths = ["src/bindings/typescript/" + name for name in names]
        attributes = subprocess.check_output(["git", "check-attr", "eol", "--", *paths], cwd=root).decode().splitlines()
        self.assertEqual(attributes, [path + ": eol: lf" for path in paths])

    def test_nested_session_build_owns_its_environment_without_inheriting_remap_flags(self):
        source = Path(__file__).with_name("reuse_candidate.py").read_text()
        calls = [node for node in ast.walk(ast.parse(source)) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Name) and node.func.id == "run" and node.args
                 and isinstance(node.args[0], ast.Constant) and node.args[0].value == "reuse-session"]
        self.assertEqual(len(calls), 1)
        environments = [keyword.value for keyword in calls[0].keywords if keyword.arg == "child_env"]
        self.assertEqual([ast.unparse(value) for value in environments], ["os.environ.copy()"])

    def test_only_successful_same_repository_native_job_is_accepted(self):
        run = {"head_sha": "a" * 40, "head_branch": "0.2.0", "event": "workflow_dispatch",
               "path": ".github/workflows/ci.yml",
               "repository": {"full_name": "xternet/svm-replay"},
               "head_repository": {"full_name": "xternet/svm-replay"}}
        job = {"name": "native-candidate (windows-2025, x64)",
               "status": "completed", "conclusion": "success"}
        self.assertEqual(require_producer(run, [job], "xternet/svm-replay", "windows-2025"), "a" * 40)
        for field, value in [("head_sha", "bad"), ("head_branch", "main"),
                             ("event", "pull_request"), ("path", "other.yml")]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                require_producer(dict(run, **{field: value}), [job], "xternet/svm-replay", "windows-2025")
        for field in ("repository", "head_repository"):
            bad = copy.deepcopy(run)
            bad[field]["full_name"] = "other/repo"
            with self.assertRaises(ValueError):
                require_producer(bad, [job], "xternet/svm-replay", "windows-2025")
        for field, value in [("name", "native-candidate (windows-11-arm, arm64)"),
                             ("status", "in_progress"), ("conclusion", "failure")]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                require_producer(run, [dict(job, **{field: value})], "xternet/svm-replay", "windows-2025")

    def test_changed_worker_wire_engine_or_dependencies_refuse_reuse(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            def git(*args):
                return subprocess.check_output(["git", *args], cwd=source).decode().strip()
            git("init", "--quiet")
            paths = ["src/cli/main.rs", "src/engine/mod.rs", "Cargo.lock",
                     "src/bindings/typescript/install.mjs",
                     "tests/interfaces/packaging/native-ci/public-inputs.json"]
            for name in paths:
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("original\n")
            git("add", ".")
            git("-c", "user.name=test", "-c", "user.email=test@example.invalid", "commit", "-qm", "producer")
            original = git("rev-parse", "HEAD")
            self.assertFalse(verify_source_reuse(source, original, False))
            sdk = source / paths[3]
            sdk.write_text("installer changed\n")
            self.assertFalse(verify_source_reuse(source, original, False))
            cli = source / paths[0]
            cli.write_text("coordinator changed\n")
            with self.assertRaisesRegex(ValueError, "coordinator"):
                verify_source_reuse(source, original, False)
            self.assertTrue(verify_source_reuse(source, original, True))
            cli.write_text("original\n")
            for name in [paths[1], paths[2], paths[4]]:
                path = source / name
                path.write_text("changed\n")
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "runtime inputs"):
                    verify_source_reuse(source, original, True)
                path.write_text("original\n")

    def test_every_retained_bundle_file_is_checked_before_repackaging(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = []
            for name in ["bin/svm-replay", "workers/reference", "workers/capture"]:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(name.encode())
                files.append({"path": name, "bytes": path.stat().st_size,
                              "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
            manifest = {"files": files}
            (root / "bundle.json").write_text(json.dumps(manifest))
            self.assertEqual(verify_bundle(root), manifest)
            (root / files[1]["path"]).write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "bundle"):
                verify_bundle(root)
            manifest["files"] = [{"path": "../outside", "bytes": 0, "sha256": "0" * 64}]
            (root / "bundle.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "bundle"):
                verify_bundle(root)

    def test_reuse_workflow_has_only_read_permissions_and_no_worker_build(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/reuse-candidate.yml").read_text()
        self.assertIn("actions: read", workflow)
        self.assertIn("contents: read", workflow)
        self.assertNotIn(": write", workflow)
        self.assertNotIn("secrets.", workflow)
        self.assertIn("github-token: ${{ github.token }}", workflow)
        self.assertIn("run-id: ${{ inputs.source_run }}", workflow)
        source = (Path(__file__).with_name("reuse_candidate.py")).read_text()
        self.assertNotIn("native_matrix.py", source)
        self.assertNotIn("--manifest-path", source)
        self.assertIn('"runtimeBuilds": 0', source)


if __name__ == "__main__":
    unittest.main()
