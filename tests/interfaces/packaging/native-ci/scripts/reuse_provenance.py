"""Authenticate the previous native producer; never infer parity from a run label."""
import json
import os
import re
import subprocess
import urllib.request
from pathlib import Path
from release_archive import digest


def require_producer(run: dict, jobs: list, repository: str, runner: str) -> str:
    if (run["repository"]["full_name"] != repository
            or run["head_repository"]["full_name"] != repository
            or run["head_branch"] != "0.2.0" or run["event"] != "workflow_dispatch"
            or run["path"] != ".github/workflows/ci.yml"
            or re.fullmatch(r"[0-9a-f]{40}", run["head_sha"]) is None):
        raise ValueError("Previous native producer identity differs")
    names = re.compile(r"native-candidate \(" + re.escape(runner) + r"(?:, (?:x64|arm64))?\)")
    selected = [job for job in jobs if names.fullmatch(job["name"])]
    if (len(selected) != 1 or selected[0]["status"] != "completed"
            or selected[0]["conclusion"] != "success"):
        raise ValueError("Previous native job did not successfully qualify this runner")
    return run["head_sha"]


def verify_source_reuse(source: Path, producer: str, refresh: bool) -> bool:
    def changed(paths):
        return subprocess.check_output(["git", "diff", "--name-only", producer, "--", *paths],
                                       cwd=source).decode().splitlines()
    differences = changed(["Cargo.toml", "Cargo.lock", "src/engine", "src/protocol", "src/store",
                           "tests/interfaces/packaging/native-ci/public-inputs.json"])
    # Owned cfg(test) fixtures are not runtime inputs; current workspace tests
    # still run whenever the coordinator is rebuilt. Never ignore production.
    if any(Path(name).name != "tests.rs" and "tests" not in Path(name).parts
           for name in differences):
        raise ValueError("Retained runtime inputs, protocol, engine or dependencies differ")
    if "tests/interfaces/packaging/native-ci/public-inputs.json" in differences:
        raise ValueError("Retained runtime inputs differ")
    coordinator_changed = bool(changed(["src/cli"]))
    if coordinator_changed and not refresh:
        raise ValueError("Changed coordinator requires its own build and affected checks")
    return coordinator_changed


def verify_bundle(root: Path) -> dict:
    manifest = json.loads((root / "bundle.json").read_bytes())
    seen = set()
    if not manifest["files"]:
        raise ValueError("Retained bundle is empty")
    for row in manifest["files"]:
        path = root / row["path"]
        if (row["path"] in seen or not path.resolve().is_relative_to(root.resolve())
                or path.is_symlink() or not path.is_file()
                or path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]):
            raise ValueError("Retained bundle file integrity differs")
        seen.add(row["path"])
    return manifest


def main() -> None:
    source = Path.cwd().resolve()
    repository = os.environ["GITHUB_REPOSITORY"]
    if repository != "xternet/svm-replay" or not os.environ["SOURCE_RUN"].isdigit():
        raise ValueError("Artifact reuse is limited to the declared repository and numeric run")
    base = f"https://api.github.com/repos/{repository}/actions/runs/{os.environ['SOURCE_RUN']}"
    def get(url):
        request = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json",
            "Authorization": "Bearer " + os.environ["GITHUB_TOKEN"]})
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    run = get(base)
    jobs = get(base + "/jobs?per_page=100")
    if jobs["total_count"] > len(jobs["jobs"]):
        raise ValueError("Producer job census exceeds this bounded validation")
    producer = require_producer(run, jobs["jobs"], repository, os.environ["SOURCE_RUNNER"])
    subprocess.run(["git", "fetch", "--no-tags", "--depth=1", "origin", producer], cwd=source, check=True)
    refresh = os.environ["REFRESH_COORDINATOR"] == "true"
    changed = verify_source_reuse(source, producer, refresh)
    provenance = {"sourceRun": int(os.environ["SOURCE_RUN"]), "sourceCommit": producer,
                  "validationCommit": os.environ["GITHUB_SHA"], "coordinatorChanged": changed,
                  "refreshCoordinator": refresh, "runtimeBuilds": 0}
    reports = source / "tests/interfaces/packaging/native-ci/reports"
    reports.mkdir()
    (reports / "reuse-producer-summary.json").write_text(json.dumps(provenance, indent=2) + "\n")


if __name__ == "__main__":
    main()
