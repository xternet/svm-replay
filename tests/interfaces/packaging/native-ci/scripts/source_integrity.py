"""Verify original source identities after qualification, before packaging."""
import json
import hashlib
import os
import tarfile
from pathlib import Path
from release_archive import digest


def verify_workers(root: Path) -> int:
    manifest = json.loads((root / "release.json").read_bytes())
    if not manifest["files"]:
        raise ValueError("Worker source manifest is empty")
    for row in manifest["files"]:
        path = root / row["path"]
        if not path.resolve().is_relative_to(root.resolve()):
            raise ValueError("Worker source path escapes owner")
        if path.is_symlink() or digest(path) != row["sha256"]:
            raise ValueError("Worker source differs: " + row["path"])
    return len(manifest["files"])


def verify_sources(run, source: Path, workers: Path, expected_commit: str,
                   expected_workers: str, archive: Path) -> dict:
    actual = run("final-source-commit", ["git", "rev-parse", "HEAD"], source, 30).read_text().strip()
    if actual != expected_commit or actual != os.environ["GITHUB_SHA"]:
        raise ValueError("Final source commit differs")
    run("final-source-diff", ["git", "diff", "--exit-code", "HEAD", "--"], source, 30)
    status = run("final-source-status", ["git", "status", "--porcelain",
                 "--untracked-files=all"], source, 30).read_text().strip()
    if status:
        raise ValueError("Final checkout is not clean: " + status)
    if digest(archive) != expected_workers:
        raise ValueError("Final worker source archive differs")
    with tarfile.open(archive) as payload:
        with payload.extractfile("worker-sources/release.json") as stream:
            original = hashlib.file_digest(stream, "sha256").hexdigest()
    if digest(workers / "release.json") != original:
        raise ValueError("Final worker source manifest differs")
    return {"sourceCommit": actual, "workerSourceFiles": verify_workers(workers),
            "workerSourcesSha256": expected_workers}
