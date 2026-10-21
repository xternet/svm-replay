"""Loader integrity/restoration only; native VM execution is a separate gate."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from cpi_fixture import portable_cpi


class CpiFixtureTests(unittest.TestCase):
    def test_shipped_source_and_binary_match_the_recorded_build(self):
        actual = Path(__file__).resolve().parents[5]
        with tempfile.TemporaryDirectory() as directory:
            _, workers, package, work, metadata, original, _ = self.prepare(directory)
            with portable_cpi(actual, workers, package, work) as receipt:
                proof = json.loads((actual / "tests/interfaces/packaging/native-ci/fixtures/session-cpi/build.json").read_bytes())
                self.assertEqual(receipt["newElfSha256"], proof["elfSha256"])
            self.assertEqual(metadata.read_bytes(), original)

    def prepare(self, directory):
        root = Path(directory)
        source, workers, work = root / "source", root / "workers", root / "work"
        fixture = source / "tests/interfaces/packaging/native-ci/fixtures/session-cpi"
        fixture.mkdir(parents=True)
        work.mkdir()
        program = source / "program.rs"
        program.write_bytes(b"synthetic loader-integrity input\n")
        elf = bytearray(64)
        elf[:6] = b"\x7fELF\x02\x01"
        elf[18:20] = (263).to_bytes(2, "little")
        (fixture / "invoke_and_ok.so").write_bytes(elf)
        digest = lambda b: hashlib.sha256(b).hexdigest()
        proof = {"schema": "svm-replay-session-cpi-test-input/v1",
                 "programSource": "program.rs", "elfSha256": digest(elf),
                 "inputs": [{"path": "program.rs", "sha256": digest(program.read_bytes())}]}
        (fixture / "build.json").write_text(json.dumps(proof))
        package = workers / "early/v2-2/Cargo.toml"
        package.parent.mkdir(parents=True)
        package.touch()
        metadata = workers / "early/shared/m9-cpi/build-manifest.json"
        metadata.parent.mkdir(parents=True)
        original = b'{"programs":{"invoke_and_ok":{"elf":"unavailable","elfSha256":"old"}}}\n'
        metadata.write_bytes(original)
        return source, workers, package, work, metadata, original, fixture

    def test_portable_fixture_is_explicit_and_original_metadata_is_restored(self):
        with tempfile.TemporaryDirectory() as directory:
            source, workers, package, work, metadata, original, _ = self.prepare(directory)
            with portable_cpi(source, workers, package, work) as receipt:
                current = json.loads(metadata.read_bytes())["programs"]["invoke_and_ok"]
                self.assertNotEqual(current["elfSha256"], "old")
                self.assertTrue(Path(current["elf"]).is_file())
                self.assertEqual(receipt["scope"], "portable-synthetic-CPI-input-not-historical")
            self.assertEqual(metadata.read_bytes(), original)
            self.assertTrue(receipt["originalRestored"])

    def test_failed_native_tests_still_restore_the_original_source_packet(self):
        with tempfile.TemporaryDirectory() as directory:
            source, workers, package, work, metadata, original, _ = self.prepare(directory)
            with self.assertRaisesRegex(RuntimeError, "native test failed"):
                with portable_cpi(source, workers, package, work):
                    raise RuntimeError("native test failed")
            self.assertEqual(metadata.read_bytes(), original)

    def test_changed_code_or_binary_is_rejected_before_any_overlay(self):
        for changed in ("source", "elf"):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory:
                source, workers, package, work, metadata, original, fixture = self.prepare(directory)
                path = source / "program.rs" if changed == "source" else fixture / "invoke_and_ok.so"
                path.write_bytes(b"changed")
                with self.assertRaisesRegex(ValueError, "differs"):
                    with portable_cpi(source, workers, package, work):
                        self.fail("corrupt input reached native execution")
                self.assertEqual(metadata.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
