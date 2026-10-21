"""Package the actual tested bundle with its source and license payloads."""
import gzip
import hashlib
import json
import tarfile
from pathlib import Path


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def regular_files(root: Path) -> list[Path]:
    if not root.is_dir() or root.is_symlink():
        raise ValueError("Archive owner must be a regular directory")
    files = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError("Archive requires regular source entries")
        if path.is_file():
            files.append(path)
    if not files:
        raise ValueError("Archive source/license owner is empty")
    return files


def pack(bundle: Path, workers: Path, licenses: Path, source: Path,
         output: Path, version: str, platform: dict) -> dict:
    owners = [(bundle, "bundle"), (workers, "runtime-sources"),
              (licenses, "coordinator-license-additions")]
    files = [(path, prefix + "/" + str(path.relative_to(root)).replace("\\", "/"))
             for root, prefix in owners for path in regular_files(root)]
    for name in ("LICENSE", "NOTICE"):
        path = source / name
        if path.is_symlink() or not path.is_file():
            raise ValueError("Missing regular coordinator license/notice")
        files.append((path, name))
    target = platform["os"] + "-" + platform["arch"]
    filename = f"svm-replay-{version}-{target}.tar.gz"
    archive = output / filename
    with archive.open("xb") as raw:
        with gzip.GzipFile(fileobj=raw, filename="", mode="wb", mtime=0, compresslevel=6) as compressed:
            with tarfile.open(fileobj=compressed, mode="w|", format=tarfile.PAX_FORMAT) as tar:
                for path, name in files:
                    info = tar.gettarinfo(str(path), arcname=name)
                    if not info.isfile():
                        raise ValueError("Archive entry changed to non-regular source")
                    info.uid, info.gid, info.uname, info.gname, info.mtime = 0, 0, "", "", 0
                    info.pax_headers = {}
                    with path.open("rb") as stream:
                        tar.addfile(info, stream)
    result = {"version": version, "platforms": [{"platform": target, "file": filename,
              "sha256": digest(archive), "bytes": archive.stat().st_size,
              "bundleSha256": digest(bundle / "bundle.json")} ]}
    with (output / "release.json").open("x") as stream:
        json.dump(result, stream, sort_keys=True, separators=(",", ":"))
    return result
