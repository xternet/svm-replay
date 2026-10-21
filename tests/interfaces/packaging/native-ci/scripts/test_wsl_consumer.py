import unittest
import inspect
from pathlib import Path
import wsl_consumer


class WslConsumerTests(unittest.TestCase):
    def test_candidate_source_is_an_explicit_commit_not_a_mutable_branch(self):
        self.assertEqual(wsl_consumer.require_commit("a" * 40), "a" * 40)
        for value in ("main", "0.2.0", "wrong", "a" * 39):
            with self.subTest(value=value), self.assertRaises(ValueError):
                wsl_consumer.require_commit(value)

    def test_wsl2_is_not_confused_with_native_windows_linux_or_wsl1(self):
        wsl_consumer.require_wsl2("6.6.87.2-microsoft-standard-WSL2\n")
        for kernel in ["6.8.0-generic", "4.4.0-Microsoft", "Windows_NT"]:
            with self.subTest(kernel=kernel), self.assertRaises(ValueError):
                wsl_consumer.require_wsl2(kernel)

    def test_wsl_is_proven_by_its_kernel_without_redundant_conversion(self):
        source = inspect.getsource(wsl_consumer.main)
        self.assertNotIn('"--set-version"', source)
        self.assertIn("require_wsl2(kernel)", source)

    def test_wsl_only_installs_downloaded_bytes_and_reuses_the_actual_consumer(self):
        root = Path(__file__).resolve().parents[5]
        workflow = (root / ".github/workflows/ci.yml").read_text()
        reusable = (root / ".github/workflows/wsl-consumer.yml").read_text()
        guest = Path(__file__).with_name("wsl_guest.sh").read_text()
        consumer = workflow.split("  wsl-consumer:\n", 1)[1].split("  wsl-probe:\n", 1)[0]
        self.assertIn("needs: native-candidate", consumer)
        self.assertIn("runs-on: windows-2025", reusable)
        self.assertIn("native-candidate-Linux-X64", reusable)
        self.assertNotIn("github-token:", reusable)
        self.assertNotIn("actions: read", reusable)
        self.assertEqual(workflow.count("(!inputs.native && !inputs.wsl_probe)"), 2)
        self.assertIn("WSL2_AVAILABLE_NOT_QUALIFIED", wsl_consumer.main.__code__.co_consts)
        self.assertIn("fresh_consumer.py", guest)
        self.assertIn("sha256sum --check", guest)
        self.assertNotIn("cargo ", guest)
        self.assertNotIn("rustup ", guest)


if __name__ == "__main__":
    unittest.main()
