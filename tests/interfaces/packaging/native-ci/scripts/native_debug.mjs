/** Real pinned-worker CLI debug protocol. Historical outputs are never command inputs. */
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { spawn } from "node:child_process";
import {knownPids, descendants, reapKnownTree} from './process_tree.mjs';
import {validateDebug} from './debug_validation.mjs';
const [binaryArg, outArg, identityArg, mode = "step"] = process.argv.slice(2);
if (!binaryArg || !outArg || !identityArg || !["step", "accounts", "eof", "sigterm", "malformed"].includes(mode))
    throw new Error("usage: bun tests/debugging/debug_cli.ts BINARY NEW_OUT PINNED_IDENTITY [step|accounts|eof|sigterm|malformed]");
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (path, value) => writeFile(path, JSON.stringify(value), { flag: "wx", mode: 0o600 });
const out = resolve(outArg);
await mkdir(out, { mode: 0o700 });
const identityBytes = await readFile(resolve(identityArg)), identity = JSON.parse(identityBytes.toString());
const catalogBytes = await readFile(identity.catalogPath);
assert.equal(digest(catalogBytes), identity.catalogSha256);
const requestBytes = await readFile(identity.requestPath), request = JSON.parse(requestBytes.toString());
const binaryBytes = await readFile(resolve(binaryArg)), binary = join(out, process.platform === "win32" ? "svm-replay.exe" : "svm-replay");
await copyFile(resolve(binaryArg), binary, constants.COPYFILE_EXCL);
await chmod(binary, 0o700);
assert.equal(digest(await readFile(binary)), digest(binaryBytes));
await save(join(out, "trace.json"), { capture: identity.policy, bounds: identity.bounds });
const start = performance.now();
const child = spawn(binary, ["simulate", "--debug", "--request", identity.requestPath, "--catalog", identity.catalogPath, "--catalog-sha256", identity.catalogSha256,
    "--trace", join(out, "trace.json"), "--data-dir", join(out, "data")], { cwd: out, stdio: ["pipe", "pipe", "pipe"] });
const stdout = [], stderr = [], events = [], commands = [];
const observerRequests = new Set();
let pending = "", chain = Promise.resolve(), interactionError, receipt;
let phase = "original-control", selected, before, after, requestId = 0, acted = false;
let accountInspection, accountDescriptor, accountEntry;
function fail(error) { if (interactionError === undefined)
    interactionError = error; child.kill("SIGTERM"); }
function send(target, action) {
    const command = { requestId: ++requestId, phase, target, action };
    commands.push(command);
    console.error(JSON.stringify({diagnostic: "command-send", elapsedMs: performance.now() - start, action: action.kind}));
    child.stdin.write(JSON.stringify(command) + "\n", error => {
        console.error(JSON.stringify({diagnostic: "command-written", elapsedMs: performance.now() - start, error: error?.message}));
        if (error) fail(error);
    });
    return command.requestId;
}
async function event(value) {
    console.error(JSON.stringify({diagnostic: "event-received", elapsedMs: performance.now() - start, kind: value.kind ?? value.schema}));
    if (value.schema === "svm-replay-receipt/v1" || value.schema === "svm-replay-cli-error/v1") {
        assert.equal(receipt, undefined, "duplicate final receipt");
        receipt = value;
        return;
    }
    assert.equal(receipt, undefined, "event after final receipt");
    events.push(value);
    if (value.kind === "response" && observerRequests.has(value.requestId)) {
        assert.equal(value.error, undefined, "observer keepalive must remain a successful read-only request");
        return;
    }
    if (value.kind === "phase-start")
        phase = value.phase;
    if (value.kind === "entry" && value.state === "STOPPED") {
        if (!acted) {
            assert.ok(child.pid);
            // Windows CIM startup can take almost the worker's 10s I/O deadline.
            // Keep the paused VM active with bounded read-only metadata requests
            // while the ownership observer runs; do not extend product deadlines.
            const keepalive = process.platform === "win32" ? setInterval(() => {
                observerRequests.add(send(value.target, {kind: "metadata"}));
            }, 1000) : undefined;
            try {
                await descendants(child.pid);
            } finally {
                if (keepalive !== undefined) clearInterval(keepalive);
            }
            assert.ok(knownPids.size > 0, "live worker ownership evidence missing");
            acted = true;
            if (mode === "eof") {
                child.stdin.end();
                return;
            }
            if (mode === "sigterm") {
                child.kill("SIGTERM");
                return;
            }
            if (mode === "malformed") {
                child.stdin.write('{"requestId":1,"requestId":2}\n');
                return;
            }
            selected = value.target;
            accountEntry = value;
            send(selected, { kind: mode === "accounts" ? "metadata" : "registers" });
            return;
        }
        if (mode === "step" || mode === "accounts")
            send(value.target, { kind: "continue" });
    }
    if (mode !== "step" && mode !== "accounts")
        return;
    if (mode === "accounts" && value.kind === "response") {
        if (value.result?.fields) {
            if (value.result.fields.account_layout !== "serialized-addresses-v1") {
                accountInspection = { status: "UNAVAILABLE", reason: "runtime does not advertise serialized-addresses-v1" };
                send(selected, { kind: "registers" });
            }
            else
                send(selected, { kind: "account-directory" });
            return;
        }
        if (value.result?.accounts) {
            assert.ok(value.result.accounts.length > 0, "advertised directory has no named account");
            accountDescriptor = value.result.accounts[0];
            assert.equal(typeof accountDescriptor.pubkey, "string");
            send(selected, { kind: "inspect-account", pubkey: accountDescriptor.pubkey, max_data_bytes: 1 });
            return;
        }
        if (value.result?.scope === "paused-vm-serialized-working-account") {
            assert.equal(value.result.pubkey, accountDescriptor.pubkey);
            assert.equal(value.result.program, accountEntry.identity.program);
            assert.equal(value.result.invocationIndex, selected.invocationIndex);
            assert.match(value.result.lamports, /^(0|[1-9][0-9]*)$/);
            assert.ok(BigInt(value.result.lamports) <= 18446744073709551615n);
            assert.ok(value.result.dataHex.length <= 2);
            assert.match(value.result.runtimeAccountState, /unavailable/);
            assert.match(value.result.committedState, /unavailable while paused/);
            accountInspection = { status: "PASS", descriptor: accountDescriptor, observed: value.result };
            send(selected, { kind: "registers" });
            return;
        }
    }
    if (value.kind === "response" && value.result?.registers) {
        if (before === undefined) {
            before = value.result.registers;
            send(selected, { kind: "step-over" });
        }
        else {
            after = value.result.registers;
            send(null, { kind: "continue-all" });
        }
    }
    if (value.kind === "stop" && value.stop.kind === "stopped") {
        const token = value.target;
        if (selected && selected.executionIndex === token.executionIndex && selected.invocationIndex === token.invocationIndex && after === undefined) {
            selected = token;
            send(token, { kind: "registers" });
        }
        else
            send(token, { kind: "continue" });
    }
}
child.stdin.on("error", fail);
child.stdout.on("data", chunk => {
    stdout.push(chunk);
    pending += chunk.toString();
    if (pending.length > 17 * 1024 * 1024) {
        fail(new Error("CLI JSONL line exceeds test bound"));
        return;
    }
    while (pending.includes("\n")) {
        const index = pending.indexOf("\n"), line = pending.slice(0, index);
        pending = pending.slice(index + 1);
        chain = chain.then(() => event(JSON.parse(line))).catch(fail);
    }
});
child.stderr.on("data", chunk => stderr.push(chunk));
const timeout = setTimeout(() => fail(new Error("debug CLI test deadline")), request.limits.timeoutMs + 15000);
const code = await new Promise((done, fail) => { child.once("error", fail); child.once("close", done); }).finally(() => clearTimeout(timeout));
await chain;
assert.equal(pending, "");
await writeFile(join(out, "stdout.jsonl"), Buffer.concat(stdout), { flag: "wx", mode: 0o600 });
await writeFile(join(out, "stderr.txt"), Buffer.concat(stderr), { flag: "wx", mode: 0o600 });
await save(join(out, "commands.json"), commands);
await save(join(out, "observed-events.json"), events);
const {unreaped, identityChecks} = await reapKnownTree();
const checks = { stdoutFraming: true, workerTreeReaped: unreaped.length === 0, originalInputUnchanged: digest(await readFile(identity.requestPath)) === digest(requestBytes), catalogUnchanged: digest(await readFile(identity.catalogPath)) === identity.catalogSha256 };
let validationError;
try {
    await validateDebug({interactionError, acted, unreaped, checks, mode,
        code, receipt, before, after, accountInspection, events});
}
catch (error) {
    validationError = String(error);
}
const summary = { schema: "svm-replay-m17-debug-cli-test/v1", status: validationError === undefined ? "PASS" : "FAIL", mode, code, caseId: request.candidate.id,
    error: validationError, interactionError: interactionError === undefined ? undefined : String(interactionError), outcome: receipt?.outcome,
    receiptPath: receipt?.receiptPath, engineReceiptPath: receipt?.error?.details?.engineReceiptPath,
    elapsedMs: performance.now() - start, eventCount: events.length, commandCount: commands.length,
    captureArtifacts: receipt?.debug?.exports?.map((item) => ({ phase: item.phase, status: item.artifact.status, bytes: item.artifact.payload.bytes, events: item.artifact.payload.events })),
    accountInspection,
    configuredBudgets: { requestTimeoutMs: request.limits.timeoutMs, captureTimeoutMs: identity.policy.limits.timeoutMs },
    knownPids: [...knownPids], identityChecks, unreaped, checks, binarySha256: digest(binaryBytes), identitySha256: digest(identityBytes), requestSha256: digest(requestBytes), catalogSha256: identity.catalogSha256 };
await save(join(out, "summary.json"), summary);
console.log(JSON.stringify(summary));
if (summary.status !== "PASS")
    process.exitCode = 1;
