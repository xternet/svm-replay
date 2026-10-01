/** Test-only checks on newly produced artifacts; never supplies execution answers. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");

export async function checkSourceReceipt(receipt: any): Promise<void> {
  const saved = JSON.parse(await readFile(receipt.receiptPath, "utf8"));
  assert.deepEqual(saved.sourceDiagnostics, receipt.sourceDiagnostics);
  const diagnostics = receipt.sourceDiagnostics;
  assert.equal(diagnostics.schema, "svm-source-diagnostics/v1");
  assert.equal(diagnostics.scope, "source-instance-cumulative");
  assert.ok(diagnostics.sources.length > 0);
  for (const row of diagnostics.sources) {
    assert.equal(row.source.kind, "captured-history");
    let local = row.diagnostics;
    if (local.cache) {
      assert.equal(local.status, "REPORTED");
      assert.equal(local.cache.failedInspections, 0);
      assert.ok(local.cache.inspectCalls >= local.cache.hits + local.cache.misses);
      local = local.underlying;
    }
    assert.equal(local.status, "LOCAL_ONLY");
    assert.deepEqual(local.transport, { requests: 0, account_reads: 0, downloaded_bytes: 0 });
  }
  const root = dirname(receipt.receiptPath);
  assert.equal(receipt.sourceObservations.file, "source-observations.json");
  assert.equal(hash(await readFile(join(root, receipt.sourceObservations.file))), receipt.sourceObservations.sha256);
  if (receipt.outcome !== "COMPLETED") return;
  assert.equal(receipt.preparedRequest.file, "prepared-request.json");
  const bytes = await readFile(join(root, receipt.preparedRequest.file));
  assert.equal(hash(bytes), receipt.preparedRequest.sha256);
  const prepared = JSON.parse(bytes.toString());
  assert.equal(prepared.schema, "svm-replay-prepared/v1");
  const fixtureBytes = await readFile(join(root, receipt.preparation.fixture.file));
  assert.equal(hash(fixtureBytes), receipt.preparation.fixture.sha256);
  assert.deepEqual(prepared.fixture, JSON.parse(fixtureBytes.toString()));
  assert.deepEqual(prepared.sourceEvidenceHashes, receipt.preparation.sourceEvidenceHashes);
  assert.deepEqual(saved.preparedRequest, receipt.preparedRequest);
}
