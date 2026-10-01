import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

test("public benchmark index is identifiers and pins, not provider snapshots", async () => {
    const index = JSON.parse(await readFile(new URL("../../../examples/historical/cases.json", import.meta.url)));
    assert.equal(index.schema, "svm-replay-benchmark-targets/v1");
    assert.equal(index.cases.length, 47);
    assert.equal(new Set(index.cases.map(row => row.id)).size, 47);
    for (const row of index.cases) {
        assert.deepEqual(Object.keys(row).sort(), ["id", "slot", "transactionIndex", "signature", "family", "executor", "prefixTransactions", "metadataPolicy", "preparedRequestSha256", "expectedOutputSha256"].sort());
        assert.ok(Number.isSafeInteger(row.slot) && row.slot > 0);
        assert.ok(Number.isSafeInteger(row.transactionIndex) && row.transactionIndex >= 0);
        assert.match(row.signature, /^[1-9A-HJ-NP-Za-km-z]{80,90}$/);
        assert.match(row.preparedRequestSha256, /^[a-f0-9]{64}$/);
        assert.match(row.expectedOutputSha256, /^[a-f0-9]{64}$/);
    }
});
