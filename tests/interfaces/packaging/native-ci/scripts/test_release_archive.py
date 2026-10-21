import hashlib
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from release_archive import pack


class ArchiveTests(unittest.TestCase):
    def fixture(self, root):
        for name in ("bundle", "workers", "licenses", "source", "output"):
            (root / name).mkdir()
        (root / "bundle/bundle.json").write_text('{"schema":"unit-archive-payload"}')
        (root / "workers/release.json").write_text('{"schema":"unit-source-payload"}')
        (root / "licenses/dependency-LICENSE").write_text("test license material\n")
        for name in ("LICENSE", "NOTICE"):
            (root / "source" / name).write_text(name + "\n")
        return {"bundle": root / "bundle", "workers": root / "workers", "licenses": root / "licenses",
                "source": root / "source", "output": root / "output", "version": "0.2.0",
                "platform": {"os": "linux", "arch": "x86_64"}}

    def test_archive_includes_real_source_and_license_payloads(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory))
            result = pack(**args)
            archive = args["output"] / result["platforms"][0]["file"]
            self.assertEqual(result["platforms"][0]["sha256"], hashlib.sha256(archive.read_bytes()).hexdigest())
            with tarfile.open(archive) as stream:
                names = stream.getnames()
                self.assertIn("runtime-sources/release.json", names)
                self.assertIn("coordinator-license-additions/dependency-LICENSE", names)
                self.assertIn("bundle/bundle.json", names)
                self.assertEqual(stream.extractfile("coordinator-license-additions/dependency-LICENSE").read(), b"test license material\n")
                self.assertTrue(all(member.isfile() for member in stream.getmembers()))
            self.assertEqual(json.loads((args["output"] / "release.json").read_bytes()), result)

    def test_empty_source_or_license_placeholders_are_refused(self):
        for key in ("workers", "licenses"):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                args = self.fixture(Path(directory))
                for path in args[key].iterdir():
                    path.unlink()
                with self.assertRaisesRegex(ValueError, "empty"):
                    pack(**args)
                self.assertFalse((args["output"] / "release.json").exists())

    def test_existing_archive_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory))
            pack(**args)
            with self.assertRaises(FileExistsError):
                pack(**args)

    def test_source_symlinks_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory))
            (args["workers"] / "bad-link").symlink_to(args["source"] / "NOTICE")
            with self.assertRaisesRegex(ValueError, "regular"):
                pack(**args)


if __name__ == "__main__":
    unittest.main()
