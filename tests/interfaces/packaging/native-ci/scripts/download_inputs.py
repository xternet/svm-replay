"""Fetch only the exact public validation inputs pinned by the reviewed checkout."""
import hashlib
import json
import re
import tarfile
import urllib.request
from pathlib import Path, PurePosixPath

BASE = "https://github.com/xternet/svm-replay/releases/download/validation-inputs-0.2.0/"
NAMES = {"qualification.tar.gz", "worker-sources.tar.gz", "session-inputs.tar.gz",
         "coordinator-licenses.tar.gz", "original-catalog.json", "qualified-catalog.json",
         "public-selection.json"}


def validate(row: dict) -> None:
    if row["file"] not in NAMES or row["url"] != BASE + row["file"]:
        raise ValueError("Input is not the declared public validation asset")
    if not isinstance(row["sha256"], str) or re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) is None:
        raise ValueError("Input digest is missing or malformed")
    if type(row["bytes"]) is not int or not 0 < row["bytes"] <= 256 * 1024 * 1024:
        raise ValueError("Input size is invalid or exceeds bound")


def copy_checked(stream, destination: Path, row: dict) -> None:
    validate(row)
    partial = destination.with_name(destination.name + ".partial")
    if destination.exists() or partial.exists():
        raise FileExistsError("Never overwrite a downloaded input")
    total, digest = 0, hashlib.sha256()
    with partial.open("xb") as output:
        while chunk := stream.read(1024 * 1024):
            total += len(chunk)
            if total > row["bytes"]:
                raise ValueError("Downloaded input exceeds its pinned size")
            digest.update(chunk)
            output.write(chunk)
    if total != row["bytes"] or digest.hexdigest() != row["sha256"]:
        raise ValueError("Downloaded input size/integrity mismatch")
    partial.rename(destination)


def extract_session(inputs: Path) -> None:
    if (inputs / "session").exists():
        raise FileExistsError("Never overwrite a session fixture")
    with tarfile.open(inputs / "session-inputs.tar.gz", "r:gz") as archive:
        members = archive.getmembers()
        seen, total = set(), 0
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != "session"
                    or "\\" in member.name or ":" in member.name or member.name in seen
                    or not (member.isfile() or member.isdir())):
                raise ValueError("Unsafe session archive entry")
            seen.add(member.name)
            total += member.size
        if total > 128 * 1024 * 1024:
            raise ValueError("Session archive exceeds the extraction bound")
        archive.extractall(inputs, members=members, filter="data")


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    manifest = json.loads((root / "public-inputs.json").read_bytes())
    if manifest["schema"] != "svm-replay-native-ci-inputs/v1":
        raise ValueError("Unknown public validation input manifest")
    rows = manifest["assets"]
    if len(rows) != len(NAMES) or {row["file"] for row in rows} != NAMES:
        raise ValueError("Public validation input census differs")
    for row in rows:
        validate(row)
    inputs = root / "inputs"
    inputs.mkdir()
    for row in rows:
        # No credentials, optional provider, retry or alternate endpoint.
        with urllib.request.urlopen(row["url"], timeout=180) as response:
            copy_checked(response, inputs / row["file"], row)
        print(json.dumps({"file": row["file"], "bytes": row["bytes"], "sha256": row["sha256"]}), flush=True)
    (inputs / "SHA256SUMS").write_text("".join(row["sha256"] + "  " + row["file"] + "\n" for row in rows))
    extract_session(inputs)


if __name__ == "__main__":
    main()
