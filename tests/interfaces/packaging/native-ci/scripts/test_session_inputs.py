import copy
import hashlib
import unittest

from session_inputs import select_records


class SessionInputTests(unittest.TestCase):
    def setUp(self):
        self.source = {"id": "recorded-provider"}
        self.record = {
            "query": {"kind": "account", "pubkey": "example", "slot": 9},
            "value": {"lamports": "123", "presence": "present", "role": "application"},
            "evidenceHashes": ["a" * 64],
        }
        self.observation = {
            "query": self.record["query"],
            "source": self.source,
            "status": "AVAILABLE",
            "valueSha256": hashlib.sha256(
                b'm11-source-value/v1\n{"lamports":"123","presence":"present"}\n').hexdigest(),
        }
        self.rows = [(self.source, self.record, "b" * 64)]

    def test_duplicate_observations_keep_one_unchanged_record(self):
        original = copy.deepcopy(self.rows)
        selected = select_records([self.observation] * 2, self.rows)
        self.assertEqual(len(selected), 1)
        self.assertEqual(selected[0]["record"], self.record)
        self.assertEqual(selected[0]["cacheBlobSha256"], "b" * 64)
        self.assertEqual(self.rows, original)

    def test_missing_record_refuses_instead_of_guessing(self):
        with self.assertRaisesRegex(ValueError, "missing exact"):
            select_records([self.observation], [])

    def test_other_provider_or_value_does_not_match(self):
        for field, value in [("source", {"id": "other"}), ("valueSha256", "c" * 64)]:
            observation = dict(self.observation, **{field: value})
            with self.assertRaisesRegex(ValueError, "missing exact"):
                select_records([observation], self.rows)

    def test_conflicting_query_values_refuse(self):
        record = dict(self.record, value={"lamports": "124", "presence": "present"})
        observation = dict(self.observation, valueSha256=hashlib.sha256(
            b'm11-source-value/v1\n{"lamports":"124","presence":"present"}\n').hexdigest())
        with self.assertRaisesRegex(ValueError, "conflicting"):
            select_records([self.observation, observation],
                           self.rows + [(self.source, record, "d" * 64)])

    def test_unavailable_observation_cannot_become_an_absent_account(self):
        with self.assertRaisesRegex(ValueError, "unavailable"):
            select_records([dict(self.observation, status="UNAVAILABLE")], self.rows)


if __name__ == "__main__":
    unittest.main()
