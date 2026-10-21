import {strict as assert} from 'node:assert';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {join, dirname} from 'node:path';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export async function validateDebug({interactionError, acted, unreaped, checks, mode,
    code, receipt, before, after, accountInspection, events}) {
    assert.equal(interactionError, undefined);
    assert.ok(acted, "worker never reached a live pause");
    assert.deepEqual(unreaped, []);
    assert.ok(checks.originalInputUnchanged && checks.catalogUnchanged);
    if (!(process.platform === "win32" && mode === "sigterm"))
        assert.ok(receipt, "final receipt missing");
    if (mode === "step" || mode === "accounts") {
        assert.equal(code, 0, JSON.stringify({ outcome: receipt.outcome, errorCode: receipt.error?.code, errorMessage: receipt.error?.message }));
        assert.equal(receipt.outcome, "COMPLETED");
        assert.equal(receipt.controlVerification.status, "PASS");
        assert.equal(receipt.verification.status, "PASS");
        const { receiptPath, ...durable } = receipt;
        assert.deepEqual(JSON.parse(await readFile(receiptPath, "utf8")), durable);
        assert.ok(before && after);
        assert.notEqual(before[11], after[11], "real PC did not advance");
        if (mode === "accounts") {
            assert.ok(accountInspection);
            if (accountInspection.status === "PASS")
                assert.equal(accountInspection.observed.pc, before[11]);
        }
        const storedEvents = await readFile(join(dirname(receipt.receiptPath), receipt.debug.events.file));
        assert.equal(digest(storedEvents), receipt.debug.events.sha256);
        // Engine persists invocation/session events; finalization notifications
        // are emitted separately after their finalized metadata has been checked.
        assert.deepEqual(JSON.parse(storedEvents.toString()), events.filter(row => !["transaction-complete", "session-complete"].includes(row.kind)), "durable events differ from live JSONL");
        const completed = events.filter(row => row.kind === "session-complete");
        assert.equal(completed.length, 1);
        assert.deepEqual(completed[0].receipt, receipt.debug.execution);
        assert.equal(events.filter(row => row.kind === "transaction-complete").length, receipt.debug.exports.length);
        assert.ok(receipt.debug.exports.length > 0, "missing live capture export");
        for (const item of receipt.debug.exports) {
            const bytes = await readFile(join(dirname(receipt.receiptPath), item.file));
            assert.equal(digest(bytes), item.sha256);
            assert.equal(item.sha256, item.artifact.payload.sha256);
            assert.equal(bytes.length, item.artifact.payload.bytes);
            assert.equal(JSON.parse(bytes.toString()).length, item.artifact.payload.events);
        }
        const output = await readFile(join(dirname(receipt.receiptPath), receipt.output.file));
        assert.equal(digest(output), receipt.output.sha256);
        checks.pcAdvanced = true;
        checks.eventsAndExportsHashBound = true;
    }
    else if (process.platform === "win32" && mode === "sigterm") {
        // Node SIGTERM uses TerminateProcess on Windows: no cooperative receipt
        // is promised. Job-object closure must still kill every observed child.
        assert.notEqual(code, 0);
        checks.forcedTerminationCleanup = true;
        checks.durableCancellation = false;
    }
    else {
        assert.notEqual(code, 0);
        assert.equal(receipt.outcome, mode === "malformed" ? "ERROR" : "CANCELLED", JSON.stringify(receipt));
        assert.equal(receipt.error.code, mode === "malformed" ? "DEBUG_INPUT" : "WORKER_CANCELLED");
        const path = mode === "malformed" ? receipt.error.details.engineReceiptPath : receipt.receiptPath;
        const stored = JSON.parse(await readFile(path, "utf8"));
        assert.equal(stored.outcome, "CANCELLED");
        assert.equal(stored.error.code, "WORKER_CANCELLED");
        checks.durableCancellation = true;
    }
}
