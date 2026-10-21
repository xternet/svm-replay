import inspect
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import native_matrix


class BuildPathTests(unittest.TestCase):
    def test_branch_label_does_not_trigger_an_unapproved_ordinary_ci_run(self):
        source = Path(os.environ['SVM_REPLAY_VALIDATION_SOURCE'])
        workflow = (source / '.github/workflows/ci.yml').read_text()
        self.assertIn("branches-ignore: ['0.2.0']", workflow)
        self.assertEqual(workflow, (source / 'examples/ci/workflow.yml').read_text())
        guide = (source / 'tests/interfaces/packaging/native-ci/README.md').read_text()
        self.assertIn('--ref 0.2.0 -f native=true', guide)
        self.assertNotIn('validation/0.2.0', guide)

    def test_remap_covers_checkout_home_and_explicit_toolchain_roots(self):
        from build_paths import build_environment
        with tempfile.TemporaryDirectory(prefix='native path ') as directory:
            root = Path(directory)
            source, work = root / 'checkout', root / 'checkout/work'
            initial = {'CARGO_HOME': str(root / 'cargo home'),
                       'RUSTUP_HOME': str(root / 'rustup home'), 'KEEP': 'present'}
            with patch.dict(os.environ, initial, clear=True):
                home = Path.home()
                env = build_environment(source, work)
            self.assertEqual(env['KEEP'], 'present')
            self.assertEqual(env['CARGO_TARGET_DIR'], str(work / 'target'))
            flags = env['CARGO_ENCODED_RUSTFLAGS'].split('\x1f')
            for path in (source, home, root / 'cargo home', root / 'rustup home'):
                self.assertTrue(any(flag.startswith('--remap-path-prefix=' + str(path.resolve()) + '=')
                                    for flag in flags), str(path))
            self.assertTrue(all(flag.startswith('--remap-path-prefix=') for flag in flags))
            self.assertNotIn('CARGO_ENCODED_RUSTFLAGS', initial)

    def test_existing_compiler_flags_are_not_silently_replaced(self):
        from build_paths import build_environment
        for key in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
            with self.subTest(key=key), patch.dict(os.environ, {key: 'unexpected'}, clear=True):
                with self.assertRaisesRegex(ValueError, 'compiler flags'):
                    build_environment(Path.cwd(), Path.cwd() / 'work')

    def test_all_native_cargo_commands_share_the_remapped_environment(self):
        self.assertIn('env = build_environment(coordinator, work)', inspect.getsource(native_matrix.main))
        import native_sessions
        self.assertIn('env = build_environment(source, target.parent)', inspect.getsource(native_sessions.qualify))


if __name__ == '__main__':
    unittest.main()
