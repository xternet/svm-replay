"""Kill an actual session parent before/after readiness; require owned descendants gone."""
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path


def processes():
    text = subprocess.check_output(["ps", "-axo", "pid=,ppid=,lstart="], text=True)
    result = {}
    for line in text.splitlines():
        pid, parent, started = line.strip().split(None, 2)
        result[int(pid)] = {"parent": int(parent), "started": started}
    return result


def descendants(table, parent):
    found = {parent}
    while True:
        added = {pid for pid, row in table.items() if row["parent"] in found}
        if added <= found:
            return {pid: table[pid] for pid in found if pid in table}
        found.update(added)


def owner(binary, descriptor, root):
    with (root / "session.log").open("xb") as log:
        child = subprocess.Popen([binary, "session", "--request", descriptor,
            "--parent-pid", str(os.getpid()), "--data-dir", str(root / "data"), "--json"],
            stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        record = root / "child.tmp"
        record.write_text(json.dumps({"pid": child.pid}))
        record.rename(root / "child.json")
        return child.wait(timeout=120)


def qualify(binary, descriptor, root, ready):
    root.mkdir()
    parent = subprocess.Popen([sys.executable, __file__, "--owner", binary, descriptor, str(root)])
    owned = {}
    try:
        deadline = time.monotonic() + 30
        while True:
            if parent.poll() is not None:
                raise RuntimeError("Session owner exited before the parent-loss test; see session.log")
            record = root / "child.json"
            log = root / "session.log"
            if record.exists():
                pid = json.loads(record.read_bytes())["pid"]
                if not ready:
                    break
                if log.exists() and log.read_bytes().endswith(b"\n"):
                    value = json.loads(log.read_bytes().splitlines()[0])
                    if value.get("schema") != "svm-historical-rpc/v1" or value.get("kind") != "ready":
                        raise RuntimeError("Bound session did not report verified readiness")
                    break
            if time.monotonic() >= deadline:
                raise TimeoutError("Parent-bound session did not start within 30 seconds")
            time.sleep(0.01)
        owned = descendants(processes(), pid)
        if pid not in owned:
            raise RuntimeError("Bound session disappeared before killing its parent")
        parent.kill()
        parent.wait(timeout=5)
        deadline = time.monotonic() + 10
        while True:
            table = processes()
            remaining = {pid: row for pid, row in owned.items()
                         if pid in table and table[pid]["started"] == row["started"]}
            if not remaining:
                return {"status": "PASS", "afterReady": ready, "owned": owned,
                        "parentSignal": "SIGKILL", "remainingProcesses": []}
            if time.monotonic() >= deadline:
                raise TimeoutError("Parent-bound CLI/worker processes survived: " + str(list(remaining)))
            time.sleep(0.05)
    finally:
        if parent.poll() is None:
            owned.update(descendants(processes(), parent.pid))
            parent.kill()
            parent.wait(timeout=5)
        table = processes()
        for pid, row in owned.items():
            if pid in table and table[pid]["started"] == row["started"]:
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError as error:
                    print(f"Parent-test cleanup: process {pid} already exited: {error}", file=sys.stderr)


if __name__ == "__main__":
    if sys.argv[1] == "--owner":
        sys.exit(owner(sys.argv[2], sys.argv[3], Path(sys.argv[4])))
    binary, descriptor, output = sys.argv[1:]
    root = Path(output)
    root.mkdir()
    rows = [qualify(binary, descriptor, root / name, ready)
            for name, ready in (("before-ready", False), ("after-ready", True))]
    result = {"status": "PASS", "checks": rows}
    (root / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
