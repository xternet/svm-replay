import unittest
import json
import tempfile
import copy
import inspect
import re
import hashlib
import os
from pathlib import Path
from release_inputs import unwrap, require_families


class ReleaseInputsTests(unittest.TestCase):
    def test_complete_discovery_unwraps_only_output(self):
        self.assertEqual(unwrap({'schema':'svm-sysvar-discovery/v1',
            'status':'COMPLETE','output':{'value':4}}), {'value':4})

    def test_partial_discovery_rejects(self):
        with self.assertRaises(ValueError):
            unwrap({'schema':'svm-sysvar-discovery/v1','status':'UNSUPPORTED','output':{}})

    def test_missing_family_rejects(self):
        with self.assertRaises(ValueError):
            require_families([{'family':'v2-3'}], ['v2-2','v2-3'])

    def test_complete_family_set_accepts(self):
        require_families([{'family':'v2-2'},{'family':'v2-3'}], ['v2-2','v2-3'])

    def fixture(self, root, cli, sdk):
        (root / "Cargo.toml").write_text(f'[package]\nname = "svm-replay"\nversion = "{cli}"\n')
        package = root / "src/bindings/typescript/package.json"
        package.parent.mkdir(parents=True)
        package.write_text(json.dumps({"name": "@xternet/svm-replay", "version": sdk}))

    def test_release_version_comes_from_matching_source_packages(self):
        from release_inputs import release_version
        for version in ["0.2.0", "0.3.0-rc.1"]:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root, version, version)
                self.assertEqual(release_version(root), version)

    def test_mismatched_release_versions_are_refused(self):
        from release_inputs import release_version
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, "0.2.0", "0.1.0")
            with self.assertRaisesRegex(ValueError, "versions differ"):
                release_version(root)

    def test_missing_sdk_version_is_not_defaulted(self):
        from release_inputs import release_version
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, "0.2.0", "0.2.0")
            (root / "src/bindings/typescript/package.json").write_text('{"name":"@xternet/svm-replay"}')
            with self.assertRaises(KeyError):
                release_version(root)

    def test_rebinding_updates_all_identity_fields_and_nothing_else(self):
        from release_inputs import rebind_request
        request = {"fixture": {"runtime": {"binding": {
            "bindingHash": "old", "slot": 123,
            "executor": {"id": "litesvm-v0.6.1-agave-2.2.20", "sourceEvidenceHash": "old-source",
                "m9Build": {"binarySha256": "old-binary", "buildHash": "old-build",
                    "capabilities": ["old-capability"], "other": "preserve"}}}}},
            "candidate": {"instructions": [1, 2]}, "limits": {"timeoutMs": 5}}
        before = copy.deepcopy(request)
        worker = {"executorSourceId": "litesvm-v0.6.1-agave-2.2.20", "sourceSha256": "new-source",
            "sha256": "new-binary", "buildHash": "new-build", "capabilities": ["new-capability"]}
        rebound = rebind_request(request, worker)
        self.assertEqual(request, before)
        binding = rebound["fixture"]["runtime"]["binding"]
        self.assertNotEqual(binding["bindingHash"], "old")
        self.assertEqual(binding["executor"]["sourceEvidenceHash"], "new-source")
        self.assertEqual(binding["executor"]["m9Build"]["capabilities"], ["new-capability"])
        self.assertEqual(binding["executor"]["m9Build"]["binarySha256"], "new-binary")
        self.assertEqual(binding["executor"]["m9Build"]["buildHash"], "new-build")
        binding["bindingHash"] = "old"
        binding["executor"]["sourceEvidenceHash"] = "old-source"
        binding["executor"]["m9Build"].update(before["fixture"]["runtime"]["binding"]["executor"]["m9Build"])
        self.assertEqual(rebound, before)

    def test_rebinding_refuses_wrong_runtime_executor(self):
        from release_inputs import rebind_request
        request = {"fixture": {"runtime": {"binding": {"executor": {"id": "litesvm-v0.6.1-agave-2.2.20"}}}}}
        with self.assertRaisesRegex(ValueError, "executor"):
            rebind_request(request, {"executorSourceId": "litesvm-v0.12.0-agave-3.1.11"})

    def test_final_workflow_covers_six_targets_without_debug_skip(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        native = workflow.split("  native-candidate:\n", 1)[1].split("  wsl-consumer:\n", 1)[0]
        runners = json.loads(re.findall(r"'(\[.*?\])'", native)[2])
        self.assertEqual(set(runners),
            {"ubuntu-22.04", "ubuntu-22.04-arm", "macos-15", "macos-15-intel",
             "windows-2025", "windows-11-arm"})
        self.assertNotIn("RELEASE_ONLY", workflow)
        self.assertIn("NATIVE_SCOPE: all", workflow)

    def test_posix_selection_does_not_readd_excluded_windows_jobs(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        for name in ("native-candidate", "clean-consumer"):
            job = workflow.split(f"  {name}:\n", 1)[1].split("    runs-on:", 1)[0]
            self.assertNotIn("exclude:", job)
            lists = [json.loads(value) for value in re.findall(r"'(\[.*?\])'", job)]
            self.assertEqual([len(value) for value in lists], [2, 4, 6])
            self.assertTrue(all(runner.startswith("ubuntu-") for runner in lists[0]))
            self.assertTrue(all(not runner.startswith("windows-") for runner in lists[0]))
            self.assertEqual(lists[1][:2], lists[0])
            self.assertEqual(lists[2][:4], lists[1])

    def test_native_archives_are_retained_and_other_consumers_run_after_an_install_failure(self):
        root = Path(os.environ["SVM_REPLAY_VALIDATION_SOURCE"])
        workflow = (root / ".github/workflows/ci.yml").read_text()
        native = workflow.split("  native-candidate:\n", 1)[1].split("  wsl-consumer:\n", 1)[0]
        archive = native.rsplit("      - uses: actions/upload-artifact@", 1)[1]
        self.assertIn("if: always()", archive)
        for name in ("wsl-consumer", "clean-consumer"):
            job = workflow.split(f"  {name}:\n", 1)[1].split("    strategy:", 1)[0]
            self.assertIn("always() && needs['native-candidate'].result != 'cancelled'", job)

    def test_full_capture_has_no_debugger_skip_switch(self):
        from native_capture import qualify
        self.assertNotIn("RELEASE_ONLY", inspect.getsource(qualify))

    def test_early_case_supplies_the_same_pinned_input_for_debugging(self):
        from release_inputs import add_early
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "inputs").mkdir()
            fixtures = root / "fixtures"
            fixtures.mkdir()
            raw = b'{"fixture":{"target":{"targetSlot":123}}}'
            row = {"id":"early","family":"v2-2","file":"early-request.json",
                "requestSha256":hashlib.sha256(raw).hexdigest(),"outputSha256":"unused-by-copy"}
            (root / "inputs/early-case.json").write_text(json.dumps(row))
            (root / "inputs/early-request.json").write_bytes(raw)
            template = {"policy":{"executionMode":"interpreter-debug"},
                "bounds":{"registerRows":4}}
            (root / "inputs/early-debug.json").write_text(json.dumps(template))
            (fixtures / "traces.json").write_text("[]")
            matrix = {"cases":[],"debug":[]}
            add_early(root, fixtures, matrix)
            self.assertEqual(matrix["debug"],[{"family":"v2-2","file":"early-request.json",
                "requestSha256":row["requestSha256"]}])
            identity = json.loads((fixtures / "debug/v2-2-identity.json").read_bytes())
            self.assertEqual(identity,template)
            self.assertEqual((fixtures / "early-request.json").read_bytes(),raw)
