import tempfile
import unittest
from pathlib import Path
from native_host import require_native_host, require_binary_arch


class NativeHostTests(unittest.TestCase):
    def test_native_host_accepts_matching_targets(self):
        for system, machine, runner, target in (
            ("linux", "x86_64", "X64", "x86_64-unknown-linux-gnu"),
            ("darwin", "arm64", "ARM64", "aarch64-apple-darwin"),
            ("win32", "ARM64", "ARM64", "aarch64-pc-windows-msvc"),
        ):
            self.assertEqual(require_native_host("host: " + target, system, machine, runner), target)

    def test_emulated_interpreter_or_compiler_is_refused(self):
        with self.assertRaisesRegex(ValueError, "runner architecture"):
            require_native_host("host: x86_64-pc-windows-msvc", "win32", "AMD64", "ARM64")
        with self.assertRaisesRegex(ValueError, "compiler host"):
            require_native_host("host: x86_64-pc-windows-msvc", "win32", "ARM64", "ARM64")

    def test_binary_cpu_is_read_from_actual_header(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "native"
            elf = bytearray(64)
            elf[:6] = b"\x7fELF\x02\x01"
            elf[18:20] = (183).to_bytes(2, "little")
            path.write_bytes(elf)
            require_binary_arch(path, "linux", "aarch64")
            with self.assertRaisesRegex(ValueError, "binary architecture"):
                require_binary_arch(path, "linux", "x86_64")
            pe = bytearray(96)
            pe[:2] = b"MZ"
            pe[60:64] = (64).to_bytes(4, "little")
            pe[64:68] = b"PE\0\0"
            pe[68:70] = (0xAA64).to_bytes(2, "little")
            path.write_bytes(pe)
            require_binary_arch(path, "windows", "aarch64")
            with self.assertRaisesRegex(ValueError, "binary architecture"):
                require_binary_arch(path, "windows", "x86_64")
            macho = bytearray(64)
            macho[:4] = bytes.fromhex("cffaedfe")
            macho[4:8] = (0x01000007).to_bytes(4, "little")
            path.write_bytes(macho)
            require_binary_arch(path, "macos", "x86_64")


if __name__ == "__main__":
    unittest.main()
