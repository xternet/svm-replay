import {strict as assert} from 'node:assert';
import {readFile, readdir} from 'node:fs/promises';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {sameProcess} from './process_identity.mjs';
const knownPids = new Set();
const windowsIdentities = new Map();
async function windowsProcesses() {
    const {stdout} = await promisify(execFile)("powershell.exe", ["-NoProfile", "-Command",
        "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,@{Name='Created';Expression={$_.CreationDate.ToUniversalTime().Ticks.ToString()}} | ConvertTo-Json -Compress"], {timeout: 60000});
    const rows = JSON.parse(stdout);
    assert.ok(Array.isArray(rows), 'process snapshot must be an array');
    return rows;
}
async function descendants(pid) {
    if (process.platform !== "linux") {
        const exec = promisify(execFile);
        let rows;
        if (process.platform === "win32") {
            const snapshot = await windowsProcesses();
            rows = snapshot.map(row => [row.ProcessId, row.ParentProcessId]);
            for (const row of snapshot) windowsIdentities.set(row.ProcessId, row);
        } else {
            const {stdout} = await exec("/bin/ps", ["-axo", "pid=,ppid="], {timeout: 5000});
            rows = stdout.trim().split("\n").map(line => line.trim().split(/\s+/).map(Number));
        }
        const visit = parent => {
            for (const [id, ppid] of rows) {
                if (ppid === parent && !knownPids.has(id)) {
                    if (process.platform === 'win32') sameProcess(windowsIdentities.get(id), windowsIdentities.get(id));
                    knownPids.add(id); visit(id);
                }
            }
        };
        visit(pid);
        return;
    }
    // Linux records a child against its spawning thread, not every process task.
    for (const tid of await readdir(`/proc/${pid}/task`)) {
        let children;
        try {
            children = await readFile(`/proc/${pid}/task/${tid}/children`, "utf8");
        }
        catch (error) {
            if (error.code === "ENOENT") {
                console.error(`ownership observer: thread ${pid}/${tid} exited`);
                continue;
            }
            throw error;
        }
        for (const child of children.trim().split(/\s+/).filter(Boolean).map(Number)) {
            assert.ok(Number.isSafeInteger(child) && child > 0);
            if (knownPids.has(child))
                continue;
            knownPids.add(child);
            await descendants(child);
        }
    }
}

export {knownPids, descendants};
export async function reapKnownTree() {
const unreaped = [];
const windowsAfter = process.platform === 'win32' ? await windowsProcesses() : undefined;
const identityChecks = [];
for (const pid of knownPids) {
    if (windowsAfter !== undefined) {
        const before = windowsIdentities.get(pid);
        const after = windowsAfter.find(row => row.ProcessId === pid);
        const alive = after !== undefined && sameProcess(before, after);
        identityChecks.push({before, after: after === undefined ? null : after, originalAlive: alive});
        if (alive) unreaped.push(pid);
        continue;
    }
    try {
        process.kill(pid, 0);
        unreaped.push(pid);
    }
    catch (error) {
        if (error.code !== "ESRCH")
            throw error;
    }
}

return {unreaped, identityChecks};
}
