"""Reconstruct a real recorded boundary, then qualify this host's session contract."""
import argparse
import json
import os
import re
import sys
from pathlib import Path

from native_matrix import verify_native_output
from probe import command
from release_inputs import rebind_request
from session_inputs import artifact, canonical, checked_file, digest
from build_paths import build_environment

EXPECTED_TESTS = [
    "initialization::real_initialized_slot_starts_before_first_vote_without_an_archived_control",
    "rpc::ordinary_native_cli_serves_verified_rpc_and_closes_its_worker",
    "transactions::ordinary_rpc_replays_signed_history_and_preserves_simulation_duplicate_reset_semantics",
    "unix::bad_identity_and_unavailable_cold_scratch_fail_without_fallback",
    "unix::cancelled_session_is_joined_before_return_and_drop_leaves_no_scratch",
    "unix::real_cached_session_is_supervised_and_reaped",
    "verified::verified_session_binds_control_and_rejects_a_rehashed_false_pass_receipt",
]


def require_session_tests(log: str) -> None:
    names = re.findall(r"^test (\S+) \.\.\. ", log, re.M)
    if (sorted(names) != sorted(EXPECTED_TESTS)
            or "test result: ok. 7 passed; 0 failed; 0 ignored;" not in log):
        raise ValueError("native session test census differs from the required seven tests")


def require_refusal(code: int, value: dict, reason: str, status: str) -> None:
    if (status != "FAIL" or code <= 0
            or value.get("error", {}).get("code") != "UNSUPPORTED_PLATFORM"
            or reason not in value["error"]["message"]):
        raise ValueError("expected explicit platform refusal, not success or an unrelated error")


def qualify(inputs: Path, binary: Path, catalog_path: Path, source: Path,
            target: Path, reports: Path, offline: bool = False) -> dict:
    reports.mkdir()
    summary = {"schema": "native-session-qualification/v1", "status": "IN_PROGRESS",
               "host": sys.platform, "checks": [], "source": str(source),
               "binarySha256": digest(binary.read_bytes()),
               "catalogSha256": digest(catalog_path.read_bytes())}
    env = build_environment(source, target.parent)
    env.update(CARGO_TARGET_DIR=str(target), NO_COLOR="1")
    env.pop("API_ALCHEMY", None)

    def save():
        (reports / "summary.json").write_bytes(canonical(summary))

    def run(name, argv, seconds=120, expected_refusal=None):
        log = reports / f"{name}.log"
        result = command([str(arg) for arg in argv], source, log, seconds, env)
        text = log.read_text(errors="strict")
        if expected_refusal is not None:
            require_refusal(result["exitCode"], json.loads(text), expected_refusal, result["status"])
            result["status"] = "PASS_EXPECTED_UNSUPPORTED"
        summary["checks"].append(dict(result, name=name))
        save()
        if result["status"] not in {"PASS", "PASS_EXPECTED_UNSUPPORTED"}:
            raise RuntimeError(f"{name} failed; see {log}")
        return text

    try:
        manifest = json.loads((inputs / "manifest.json").read_bytes())
        if manifest["schema"] != "release-portable-session-inputs/v1":
            raise ValueError("unknown session input schema")
        for row in manifest["files"]:
            artifact(inputs, row)
        summary["inputManifestSha256"] = digest((inputs / "manifest.json").read_bytes())
        capture_path = inputs / "capture.json"
        capture = json.loads(capture_path.read_bytes())
        for row in capture["entries"]:
            artifact(inputs, row)
        original = json.loads((inputs / "prepared.json").read_bytes())
        original_result = (inputs / "original-output.json").read_bytes()
        catalog = json.loads(catalog_path.read_bytes())
        worker = next(row for row in catalog["workers"] if row["family"] == original["family"])
        worker_path = catalog_path.parent / worker["file"]
        checked_file(worker_path, worker["sha256"])
        rebound = rebind_request(original, worker)
        binding = rebound["fixture"]["runtime"]["binding"]
        binding_path, candidate_path = reports / "binding.json", reports / "candidate.json"
        binding_path.write_bytes(canonical(binding))
        candidate_path.write_bytes(canonical(original["candidate"]))
        request_path = reports / "historical-request.json"
        run("build-request", [binary, "request", "--candidate", candidate_path,
            "--runtime-binding", binding_path, "--family", original["family"],
            "--genesis-hash", capture["identity"]["genesisHash"],
            "--request-id", "portable-session-real-history", "--output", request_path])
        result = json.loads(run("reconstruct", [binary, "simulate", "--request", request_path,
            "--catalog", catalog_path, "--catalog-sha256", summary["catalogSha256"],
            "--source", capture_path, "--source-sha256", digest(capture_path.read_bytes()),
            "--data-dir", reports / "data"], seconds=300))
        if (result["outcome"] != "COMPLETED" or result["currentStateFallback"] is not False
                or result["verification"]["status"] != "PASS"
                or result["controlVerification"]["status"] != "PASS"):
            raise ValueError("source reconstruction did not pass original-control verification")
        receipt_path = Path(result["receiptPath"])
        receipt = json.loads(receipt_path.read_bytes())
        if receipt["scope"] != "source-reconstructed-replay/v1":
            raise ValueError("source preparation proof missing")
        output = artifact(receipt_path.parent, receipt["output"])
        verify_native_output(output, receipt["output"]["sha256"], binding["bindingHash"],
                             original["fixture"]["runtime"]["binding"]["bindingHash"],
                             digest(original_result))
        prepared = json.loads(artifact(receipt_path.parent, receipt["preparedRequest"]))
        fixture_path = reports / "fixture.json"
        fixture_path.write_bytes(canonical(prepared["fixture"]))
        descriptor = reports / "session-input.json"
        descriptor.write_bytes(canonical({
            "schema": "svm-historical-session-input/v1",
            "receiptPath": str(receipt_path), "receiptSha256": digest(receipt_path.read_bytes()),
            "workerPath": str(worker_path), "after": True,
        }))
        summary["receiptPath"] = str(receipt_path)
        summary["receiptSha256"] = digest(receipt_path.read_bytes())
        summary["workerSha256"] = worker["sha256"]
        if os.name == "nt":
            run("windows-session-refusal", [binary, "session", "--request", descriptor,
                "--data-dir", reports / "windows-refusal", "--json"],
                expected_refusal="require Unix")
            summary["transport"] = "UNSUPPORTED_PLATFORM"
        else:
            env.update({
                "SVM_SESSION_FIXTURE": str(fixture_path),
                "SVM_SESSION_FIXTURE_SHA256": digest(fixture_path.read_bytes()),
                "SVM_SESSION_RECEIPT": str(receipt_path),
                "SVM_SESSION_RECEIPT_SHA256": digest(receipt_path.read_bytes()),
                "SVM_SESSION_WORKER": str(worker_path),
                "SVM_SESSION_WORKER_SHA256": worker["sha256"],
            })
            argv = ["cargo", "+1.94.0", "test", "--locked", "--release", "--jobs", "4"]
            if offline:
                argv.append("--offline")
            test_cli = target / "release" / binary.name
            checked_file(test_cli, summary["binarySha256"])
            argv.extend(["--test", "native_session", "--", "--ignored",
                         "--test-threads=1"])
            require_session_tests(run("unix-session-tests", argv, seconds=900))
            checked_file(test_cli, summary["binarySha256"])
            summary["transport"] = "PASS"
            summary["tests"] = list(EXPECTED_TESTS)
        if sys.platform != "linux":
            run("parent-lifetime-refusal", [binary, "session", "--request", descriptor,
                "--data-dir", reports / "parent-refusal", "--json", "--parent-pid", os.getpid()],
                expected_refusal="qualified on Linux only")
            summary["sdkParentLifetime"] = "UNSUPPORTED_PLATFORM"
        else:
            summary["sdkParentLifetime"] = "covered separately by process integration tests"
        if digest(binary.read_bytes()) != summary["binarySha256"]:
            raise ValueError("session test build changed the qualified CLI binary")
        summary["status"] = "PASS"
        return summary
    except Exception as error:
        summary["status"] = "FAIL"
        summary["error"] = str(error)
        raise
    finally:
        save()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("inputs", "binary", "catalog", "source", "target", "reports"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    print(json.dumps(qualify(args.inputs.resolve(), args.binary.resolve(), args.catalog.resolve(),
                            args.source.resolve(), args.target.resolve(), args.reports.resolve(),
                            args.offline)))
