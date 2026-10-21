import hashlib
import io
import tempfile
import unittest
from pathlib import Path
from download_inputs import validate, copy_checked, BASE


class DownloadTests(unittest.TestCase):
    def row(self):
        return {"file": "qualification.tar.gz", "url": BASE + "qualification.tar.gz",
                "sha256": hashlib.sha256(b"verified").hexdigest(), "bytes": 8}

    def test_bound_public_url_and_pin_are_required(self):
        validate(self.row())
        for field, value in (("url", "https://example.com/data"),
                             ("file", "../outside"), ("sha256", "bad"), ("bytes", -1)):
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate(dict(self.row(), **{field: value}))

    def test_copy_hashes_actual_stream_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "data"
            copy_checked(io.BytesIO(b"verified"), path, self.row())
            self.assertEqual(path.read_bytes(), b"verified")
            with self.assertRaises(FileExistsError):
                copy_checked(io.BytesIO(b"verified"), path, self.row())

    def test_truncated_oversized_and_changed_content_are_refused(self):
        for data in (b"short", b"oversized-data", b"modified"):
            with self.subTest(data=data), tempfile.TemporaryDirectory() as directory:
                with self.assertRaisesRegex(ValueError, "size|integrity"):
                    copy_checked(io.BytesIO(data), Path(directory) / "data", self.row())


if __name__ == "__main__":
    unittest.main()
