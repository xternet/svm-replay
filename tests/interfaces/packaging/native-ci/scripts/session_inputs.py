"""Export exact recorded observations; never fabricate a source receipt."""
import argparse
import hashlib
import json
import sqlite3
from pathlib import Path
from typing import Any


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def observation_value_pin(record: dict) -> str:
    value = record["value"]
    if record["query"]["kind"] == "account":
        value = {key: item for key, item in value.items() if key != "role"}
    return digest(b"m11-source-value/v1\n" + canonical(value) + b"\n")


def select_records(observations: list, rows: list) -> list:
    selected = {}
    for observation in observations:
        if observation["status"] != "AVAILABLE":
            raise ValueError("unavailable observation cannot be exported as an account image")
        key = canonical(observation["query"])
        matches = [
            (record, blob_pin) for source, record, blob_pin in rows
            if source == observation["source"]
            and record["query"] == observation["query"]
            and observation_value_pin(record) == observation["valueSha256"]
        ]
        if not matches:
            raise ValueError(f"missing exact recorded query: {digest(key)}")
        record, blob_pin = matches[0]
        if key in selected and selected[key]["record"]["value"] != record["value"]:
            raise ValueError(f"conflicting values for recorded query: {digest(key)}")
        selected[key] = {"record": record, "cacheBlobSha256": blob_pin}
    return [selected[key] for key in sorted(selected)]


def checked_file(path: Path, pin: str) -> bytes:
    raw = path.read_bytes()
    if digest(raw) != pin:
        raise ValueError(f"input digest mismatch: {path}")
    return raw


def artifact(root: Path, descriptor: dict) -> bytes:
    path = root / descriptor["file"]
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("receipt artifact escapes its directory")
    return checked_file(path, descriptor["sha256"])


def export(seed: Path, receipt_path: Path, receipt_pin: str, destination: Path) -> dict:
    receipt = json.loads(checked_file(receipt_path, receipt_pin))
    if (receipt["scope"] != "source-reconstructed-replay/v1"
            or receipt["outcome"] != "COMPLETED"
            or receipt["currentStateFallback"] is not False
            or receipt["verification"]["status"] != "PASS"):
        raise ValueError("a verified source-reconstructed receipt is required")
    root = receipt_path.parent
    preparation = json.loads(artifact(root, receipt["sourcePreparation"]))
    observations_path = root / "source-observations.json"
    observations = json.loads(checked_file(
        observations_path, preparation["sourceObservationsSha256"]))
    prepared = artifact(root, receipt["preparedRequest"])
    original_output = artifact(root, receipt["output"])
    queries = {canonical(row["query"]) for row in observations}
    records = []
    db_path = seed / "index.sqlite"
    # immutable=1 prevents creating WAL/SHM sidecars in the sealed evidence.
    with sqlite3.connect(db_path.resolve().as_uri() + "?mode=ro&immutable=1", uri=True) as db:
        for key, pin in db.execute("SELECT key,hash FROM entries WHERE namespace='raw'"):
            entry = json.loads(checked_file(seed / "blobs" / pin, pin))
            if key.startswith("m17-raw-source/v1/"):
                source, record = entry["source"], entry["record"]
            elif key.startswith("discovery/"):
                source, record = entry["identity"]["source"], entry["value"]
            else:
                raise ValueError(f"unknown raw cache schema: {key.split('/')[0]}")
            if isinstance(record, dict) and "query" in record:
                if canonical(record["query"]) in queries:
                    records.append((source, record, pin))
    selected = select_records(observations, records)
    destination.mkdir()
    (destination / "records").mkdir()
    entries, origins = [], []
    for row in selected:
        raw = canonical(row["record"])
        pin = digest(raw)
        relative = f"records/{pin}.json"
        (destination / relative).write_bytes(raw)
        entries.append({"query": row["record"]["query"], "file": relative, "sha256": pin})
        origins.append({"recordSha256": pin, "cacheBlobSha256": row["cacheBlobSha256"]})
    slots = [row["query"]["slot"] for row in entries]
    capture = {
        "schema": "m11-captured-source/v1",
        "identity": {
            "id": "release-session-recorded-inputs", "version": "v1", "kind": "captured-history",
            "genesisHash": preparation["genesisHash"],
            "coverage": {"firstSlot": min(slots), "lastSlot": max(slots), "completeness": "partial"},
            "capabilities": sorted({row["query"]["kind"] for row in entries}),
        },
        "entries": entries,
    }
    files = {
        "capture.json": canonical(capture), "prepared.json": prepared,
        "original-output.json": original_output,
    }
    for name, raw in files.items():
        (destination / name).write_bytes(raw)
    manifest = {
        "schema": "release-portable-session-inputs/v1",
        "sourceReceiptSha256": receipt_pin,
        "sourceObservationsSha256": digest(observations_path.read_bytes()),
        "observationCount": len(observations), "recordCount": len(entries),
        "origins": origins, "networkUsed": False,
        "files": [{"file": name, "sha256": digest(raw)} for name, raw in files.items()],
        "publicationApproved": False,
    }
    (destination / "manifest.json").write_bytes(canonical(manifest))
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--receipt-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(export(args.seed, args.receipt, args.receipt_sha256, args.output)))
