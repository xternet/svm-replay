import ast
import inspect
import unittest
from pathlib import Path
import native_matrix
from native_matrix import digest, encoded, verify_native_output


class ReleaseBuildTests(unittest.TestCase):
    def setUp(self):
        self.tree = ast.parse(inspect.getsource(native_matrix.main))
        commands = [node.value for node in ast.walk(self.tree)
                    if isinstance(node, ast.Assign)
                    and any(isinstance(target, ast.Name) and target.id == 'cargo'
                            for target in node.targets)]
        self.assertEqual(len(commands), 1)
        self.cargo = ast.literal_eval(commands[0])

    def test_workers_use_release_build(self):
        self.assertIn('build', self.cargo)
        self.assertEqual(self.cargo.count('--release'), 1)

    def test_layout_is_checked_before_external_sources_are_extracted(self):
        calls = [node for node in ast.walk(self.tree) if isinstance(node, ast.Call)]
        layout = [node for node in calls if isinstance(node.func, ast.Name)
                  and node.func.id == 'run' and node.args
                  and isinstance(node.args[0], ast.Constant)
                  and node.args[0].value == 'repository-layout']
        extraction = [node for node in calls if isinstance(node.func, ast.Attribute)
                      and node.func.attr == 'extractall']
        self.assertEqual(len(layout), 1)
        self.assertTrue(extraction)
        self.assertLess(layout[0].lineno, min(node.lineno for node in extraction),
                        'external worker Rust files must not pollute coordinator layout checks')

    def test_sdk_native_projection_uses_the_cli_built_by_this_job(self):
        source = inspect.getsource(native_matrix.main)
        selected = "env['SVM_REPLAY_BOUNDARY_BINARY'] = str(binary)"
        self.assertIn(selected, source)
        self.assertLess(source.index(selected), source.index("run('sdk-unit-tests'"))

    def test_cli_inherits_release_without_duplicate_flag(self):
        calls = [node for node in ast.walk(self.tree)
                 if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
                 and node.func.id == 'run' and node.args
                 and isinstance(node.args[0], ast.Constant)
                 and node.args[0].value == 'cli-build']
        self.assertEqual(len(calls), 1)
        argv = calls[0].args[1]
        self.assertIsInstance(argv, ast.BinOp)
        self.assertEqual(argv.left.id, 'cargo')
        self.assertEqual((self.cargo + ast.literal_eval(argv.right)).count('--release'), 1)

    def test_packaged_worker_paths_use_release_outputs(self):
        for name in ('native_matrix.py', 'native_capture.py'):
            source = Path(native_matrix.__file__).with_name(name).read_text()
            with self.subTest(script=name):
                self.assertNotIn('target/debug/', source)
                self.assertIn('target/release/', source)

    def test_rebuilt_catalog_binds_to_the_reviewed_runtime_profile(self):
        assignments = [node.value for node in ast.walk(self.tree)
                       if isinstance(node, ast.Assign) and any(
                           isinstance(target, ast.Name) and target.id == 'descriptor'
                           for target in node.targets)]
        self.assertEqual(len(assignments), 1)
        reviewed_source = digest(b'original reviewed VM source inventory')
        reviewed = {'family': 'v2-2', 'executorSourceId': 'reviewed-executor',
                    'sourceSha256': reviewed_source}
        build = {'family': reviewed['family'], 'inputFiles': [],
                 'retainedSourceSha256': reviewed_source,
                 'portableInputSha256': digest(encoded([]) + b'\n')}
        registry = {'executors': [{'id': reviewed['executorSourceId'],
                    'packageName': reviewed['family'], 'reviewedSourceSha256': reviewed_source}]}
        expression = ast.Expression(assignments[0])
        descriptor = eval(compile(expression, '<native catalog producer>', 'eval'),
                          vars(native_matrix), dict(build=build, reviewed=reviewed,
                          registry=registry, destination=Path('worker'),
                          worker_pin=digest(b'new native binary'), build_pin=digest(b'new build')))
        self.assertEqual(descriptor['sourceSha256'], registry['executors'][0]['reviewedSourceSha256'])
        self.assertNotEqual(descriptor['sourceSha256'], build['portableInputSha256'])


class NativeOutputTests(unittest.TestCase):
    def setUp(self):
        self.original = {'runtimeBindingHash': 'old', 'original': {'balance': '100', 'cu': 37}}
        self.native = dict(self.original, runtimeBindingHash='native')
        self.expected = digest(encoded(self.original))

    def check(self, value, pin=None):
        raw = encoded(value)
        verify_native_output(raw, digest(raw) if pin is None else pin, 'native', 'old', self.expected)

    def test_only_binding_change_is_accepted(self):
        self.check(self.native)

    def test_execution_change_is_rejected(self):
        for field in ('balance', 'cu'):
            value = dict(self.native, original=dict(self.native['original'], **{field: 99}))
            with self.assertRaisesRegex(RuntimeError, 'historical execution'):
                self.check(value)

    def test_wrong_native_identity_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'native output binding'):
            self.check(dict(self.native, runtimeBindingHash='other'))

    def test_receipt_corruption_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'receipt output digest'):
            self.check(self.native, 'bad')
