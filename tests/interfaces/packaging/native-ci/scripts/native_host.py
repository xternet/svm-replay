"""Refuse emulated toolchains and incorrectly labelled native executable headers."""
import re
import subprocess
from pathlib import Path

ARCHES = {"x86_64": "x86_64", "AMD64": "x86_64", "X64": "x86_64",
          "aarch64": "aarch64", "arm64": "aarch64", "ARM64": "aarch64"}


def require_native_host(text: str, system: str, machine: str, runner: str) -> str:
    architecture = ARCHES[machine]
    if architecture != ARCHES[runner]:
        raise ValueError("Interpreter differs from native runner architecture")
    suffix = {"linux": "unknown-linux-gnu", "darwin": "apple-darwin", "win32": "pc-windows-msvc"}[system]
    expected = architecture + "-" + suffix
    hosts = re.findall(r"^host: (\S+)$", text, re.MULTILINE)
    if hosts != [expected]:
        raise ValueError(f"Native compiler host differs: expected {expected}, found {hosts}")
    return expected


def require_glibc_version(text: str) -> str:
    versions = {tuple(int(part) for part in value.split("."))
                for value in re.findall(r"\bGLIBC_([0-9]+(?:\.[0-9]+)+)\b", text)}
    if not versions or max(versions) > (2, 35):
        raise ValueError("Native Linux executable exceeds or lacks the glibc 2.35 baseline")
    return ".".join(str(part) for part in max(versions))


def require_binary_arch(path: Path, system: str, architecture: str,
                        linux_baseline: bool = False) -> None:
    observed = None
    with path.open("rb") as stream:
        header = stream.read(64)
        if system == "linux" and header[:6] == b"\x7fELF\x02\x01":
            observed = {62: "x86_64", 183: "aarch64"}.get(int.from_bytes(header[18:20], "little"))
        elif system == "macos" and header[:4] == bytes.fromhex("cffaedfe"):
            observed = {0x01000007: "x86_64", 0x0100000C: "aarch64"}.get(int.from_bytes(header[4:8], "little"))
        elif system == "windows" and header[:2] == b"MZ" and len(header) == 64:
            offset = int.from_bytes(header[60:64], "little")
            if offset > 16 * 1024 * 1024:
                raise ValueError("Native binary PE header offset exceeds bound")
            stream.seek(offset)
            coff = stream.read(6)
            if coff[:4] == b"PE\0\0":
                observed = {0x8664: "x86_64", 0xAA64: "aarch64"}.get(int.from_bytes(coff[4:6], "little"))
    if observed != architecture:
        raise ValueError(f"Native binary architecture differs for {path.name}: expected {architecture}, found {observed}")
    if linux_baseline:
        if system != "linux":
            raise ValueError("glibc qualification requires a native Linux executable")
        text = subprocess.check_output(["readelf", "--version-info", str(path)],
                                       stderr=subprocess.STDOUT, text=True, timeout=30)
        require_glibc_version(text)
