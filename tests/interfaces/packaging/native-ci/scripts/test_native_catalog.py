import copy
import hashlib
import json
import unittest
from native_catalog import reference_descriptor


class NativeCatalogTests(unittest.TestCase):
    def setUp(self):
        self.source = hashlib.sha256(b'reviewed source receipt').hexdigest()
        self.build = {'family': 'v2-2', 'inputFiles': [],
                      'retainedSourceSha256': self.source,
                      'portableInputSha256': hashlib.sha256(b'[]\n').hexdigest()}
        self.reviewed = {'family': 'v2-2', 'executorSourceId': 'reviewed-vm',
                         'sourceSha256': self.source, 'sha256': 'old-binary',
                         'buildHash': 'old-build', 'capabilities': ['existing']}
        self.registry = {'executors': [{'id': 'reviewed-vm', 'packageName': 'v2-2',
                                        'reviewedSourceSha256': self.source}]}

    def descriptor(self):
        return reference_descriptor(self.build, self.reviewed, self.registry,
                                    'new-worker', 'new-binary', 'new-build')

    def test_only_native_build_identity_and_path_change(self):
        before = copy.deepcopy(self.reviewed)
        result = self.descriptor()
        self.assertEqual(result, dict(before, file='new-worker', sha256='new-binary',
                                      buildHash='new-build'))
        self.assertEqual(self.reviewed, before)
        self.assertNotEqual(result['sourceSha256'], self.build['portableInputSha256'])

    def test_portable_inventory_corruption_is_not_accepted(self):
        self.build['inputFiles'].append({'path': 'unexpected.rs'})
        with self.assertRaisesRegex(ValueError, 'portable worker input'):
            self.descriptor()

    def test_source_and_family_disagreement_are_not_accepted(self):
        for owner, key in [(self.build, 'retainedSourceSha256'),
                           (self.build, 'family'), (self.reviewed, 'sourceSha256'),
                           (self.registry['executors'][0], 'reviewedSourceSha256'),
                           (self.registry['executors'][0], 'packageName')]:
            with self.subTest(key=key):
                original = owner[key]
                owner[key] = 'wrong'
                with self.assertRaisesRegex(ValueError, 'reviewed runtime profile'):
                    self.descriptor()
                owner[key] = original

    def test_missing_or_duplicate_executor_is_not_accepted(self):
        for executors in [[], self.registry['executors'] * 2]:
            with self.subTest(executors=len(executors)):
                registry = dict(self.registry, executors=executors)
                with self.assertRaisesRegex(ValueError, 'reviewed runtime profile'):
                    reference_descriptor(self.build, self.reviewed, registry, 'worker', 'bin', 'build')
