"""Explicit synthetic CPI input overlay; never alter historical controls or worker code."""
from contextlib import contextmanager
import hashlib
import json
import re
from pathlib import Path


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


@contextmanager
def portable_cpi(source: Path, workers: Path, package: Path, work: Path):
    fixture = source / "tests/interfaces/packaging/native-ci/fixtures/session-cpi"
    proof = json.loads((fixture / "build.json").read_bytes())
    if proof["schema"] != "svm-replay-session-cpi-test-input/v1":
        raise ValueError("Unknown synthetic CPI fixture provenance")
    for row in proof["inputs"]:
        path = (source / row["path"]).resolve(strict=True)
        if not path.is_relative_to(source.resolve()):
            raise ValueError("Synthetic CPI source escapes its owner")
        if digest(path.read_bytes().replace(b"\r\n", b"\n")) != row["sha256"]:
            raise ValueError("Synthetic CPI source differs: " + row["path"])
    elf = fixture / "invoke_and_ok.so"
    binary = elf.read_bytes()
    if digest(binary) != proof["elfSha256"]:
        raise ValueError("Synthetic CPI ELF differs")
    if (len(binary) < 64 or len(binary) > 1024 * 1024 or binary[:6] != b"\x7fELF\x02\x01"
            or int.from_bytes(binary[18:20], "little") != 263
            or int.from_bytes(binary[48:52], "little") != 0):
        raise ValueError("Synthetic CPI input must be a bounded SBPFv0 ELF")
    code = (source / proof["programSource"]).resolve(strict=True)
    if (not code.is_relative_to(source.resolve())
            or proof["programSource"] not in [row["path"] for row in proof["inputs"]]):
        raise ValueError("Synthetic CPI program source is not pinned")
    canonical = code.read_bytes().replace(b"\r\n", b"\n")
    normalized = work / "session-cpi-source.txt"
    if normalized.exists():
        if normalized.read_bytes() != canonical:
            raise ValueError("Synthetic CPI source copy differs")
    else:
        with normalized.open("xb") as stream:
            stream.write(canonical)
    if not package.resolve(strict=True).is_relative_to(workers.resolve()):
        raise ValueError("Worker test manifest escapes the extracted source packet")
    metadata = package.parent.parent / "shared/m9-cpi/build-manifest.json"
    if metadata.is_symlink() or not metadata.resolve(strict=True).is_relative_to(workers.resolve()):
        raise ValueError("Worker fixture metadata escapes its source packet")
    original = metadata.read_bytes()
    mode = metadata.stat().st_mode & 0o777
    value = json.loads(original)
    old = value["programs"]["invoke_and_ok"]
    value["programs"]["invoke_and_ok"] = dict(old, source=str(normalized),
        sourceSha256=digest(canonical), elf=str(elf), elfSha256=proof["elfSha256"])
    value["ciSyntheticCpiInput"] = proof
    receipt = {"scope": "portable-synthetic-CPI-input-not-historical",
               "originalManifestSha256": digest(original),
               "newElfSha256": proof["elfSha256"], "newSourceSha256": digest(canonical),
               "oldInput": old, "replacementBuild": proof, "originalRestored": False}
    try:
        metadata.chmod(mode | 0o200)
        metadata.write_text(json.dumps(value, indent=2) + "\n")
        yield receipt
    finally:
        metadata.write_bytes(original)
        metadata.chmod(mode)
        if metadata.read_bytes() != original:
            raise ValueError("Original worker fixture metadata was not restored")
        receipt["originalRestored"] = True


def run_native_session_tests(run, source: Path, workers: Path, build: dict, work: Path) -> dict:
    family = build["family"]
    package = workers / build["manifest"]
    with portable_cpi(source, workers, package, work) as receipt:
        log = run("session-unit-" + family, ["cargo", "+1.94.0", "test", "--release",
            "--locked", "--jobs", "4", "--target-dir", str(work / "native-target"),
            "--manifest-path", str(package), "--bin", "m6-executor-" + family,
            "m12_session", "--", "--test-threads=2"], workers, 900)
    census = re.search(r"test result: ok\. ([0-9]+) passed; 0 failed; ([0-9]+) ignored;", log.read_text())
    if census is None or int(census[1]) == 0:
        raise RuntimeError("native session unit census missing or empty: " + family)
    return {"family": family, "passed": int(census[1]), "ignored": int(census[2]),
            "scope": "synthetic native session tests", "cpiTestInput": receipt}
