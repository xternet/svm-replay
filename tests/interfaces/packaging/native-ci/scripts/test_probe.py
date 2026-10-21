import io
import os
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

from probe import assessment_host, command, safe_members, first_blocker


class ProbeTests(unittest.TestCase):
    def test_stages_stop_at_first_failure(self):
        calls = []
        def run(name, *args):
            calls.append(name)
            return name != 'launch'
        result = first_blocker(run, [('build',), ('launch',), ('replay',)])
        self.assertEqual(result, 'launch')
        self.assertEqual(calls, ['build', 'launch'])
        self.assertIsNone(first_blocker(lambda *args: True, [('build',)]))

    def test_only_assessment_targets_are_admitted(self):
        for operating_system, architecture in [('darwin', 'arm64'), ('darwin', 'x86_64'),
                                                ('win32', 'AMD64'), ('win32', 'ARM64'),
                                                ('linux', 'aarch64')]:
            self.assertTrue(assessment_host(operating_system, architecture))
        self.assertFalse(assessment_host('linux', 'x86_64'))
        self.assertFalse(assessment_host('freebsd', 'x86_64'))

    def test_archives_reject_traversal_and_links(self):
        for name, kind in [('../escape', tarfile.REGTYPE), ('/absolute', tarfile.REGTYPE),
                           ('link', tarfile.SYMTYPE), ('link', tarfile.LNKTYPE)]:
            stream = io.BytesIO()
            with tarfile.open(fileobj=stream, mode='w') as archive:
                entry = tarfile.TarInfo(name)
                entry.type = kind
                archive.addfile(entry)
            stream.seek(0)
            with tarfile.open(fileobj=stream) as archive:
                with self.assertRaises(ValueError):
                    safe_members(archive)

    def test_failure_and_timeout_are_not_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = command([sys.executable, '-c', 'raise SystemExit(7)'], root,
                             root / 'failure.log', 5, os.environ)
            self.assertEqual(result['status'], 'FAIL')
            self.assertEqual(result['exitCode'], 7)
            result = command([sys.executable, '-c', 'import time; time.sleep(10)'], root,
                             root / 'timeout.log', 0.1, os.environ)
            self.assertEqual(result['status'], 'TIMEOUT')
            self.assertNotEqual(result['exitCode'], 0)


if __name__ == '__main__':
    unittest.main()
