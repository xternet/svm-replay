import inspect
import unittest
from pathlib import Path
import native_matrix


class PublicContractTests(unittest.TestCase):
    def test_uses_explicit_source_and_declared_seven_family_selection(self):
        source = inspect.getsource(native_matrix.main)
        self.assertIn('SVM_REPLAY_VALIDATION_SOURCE', source)
        self.assertIn('qualification.tar.gz', source)
        self.assertIn('public-selection.json', source)
        self.assertNotIn('inputs/corpus', source)
        self.assertNotIn('Original owner-approved WSL corpus archive', source)
        self.assertNotIn('add_early(ROOT', source)

    def test_native_capture_sdk_and_session_gates_are_not_removed(self):
        source = inspect.getsource(native_matrix.main)
        for required in ('qualify_sessions(', 'qualify(run, cargo,', 'sdk-six-traces',
                         'verification', 'controlVerification', 'verify_native_output('):
            self.assertIn(required, source)

    def test_demo_uses_actual_sources_licenses_and_new_native_installer(self):
        source = Path(native_matrix.__file__).with_name('demo_qualification.py').read_text()
        self.assertNotIn('info.type = tarfile.DIRTYPE', source)
        self.assertIn('from release_archive import pack', source)
        self.assertIn('native-install-summary.json', source)


if __name__ == "__main__":
    unittest.main()
