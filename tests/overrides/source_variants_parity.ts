import { requiredPath } from "../harness/paths";
/** Post-execution oracle only. Never passes archived outputs into reconstruction. */
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

const [runArg] = process.argv.slice(2);
if (!runArg) throw new Error("usage: bun tests/overrides/source_variants_parity.ts COMPLETED_NATIVE_VARIANTS_RUN");
const digest = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const read = async (path: string) => JSON.parse(await readFile(path, "utf8"));
const current = await read(join(resolve(runArg), "summary.json"));
assert.equal(current.status, "PASS");
const previousRoot = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m15-connections-S4vQea");
const previous = await read(join(previousRoot, "summary.json"));
assert.equal(previous.status, "PASS");
for (const label of ["original", "replacement"]) {
  const row = current.results.find((row: any) => row.label === label);
  assert.ok(row, `missing native ${label}`);
  const receipt = await read(row.receiptPath);
  assert.equal(receipt.outcome, "COMPLETED");
  const output = await readFile(join(dirname(row.receiptPath), receipt.output.file));
  assert.equal(digest(output), receipt.output.sha256);
  const prior = previous.results.find((row: any) => row.label === label);
  const priorReceipt = await readFile(join(previousRoot, label, "receipt.json"));
  assert.equal(digest(priorReceipt), prior.receiptSha256);
  const priorOutput = await readFile(join(previousRoot, label, "execution/result.json"));
  assert.equal(digest(priorOutput), JSON.parse(priorReceipt.toString()).execution.output.sha256);
  assert.deepEqual(JSON.parse(output.toString()), JSON.parse(priorOutput.toString()));
  console.log(JSON.stringify({ label, status: "PASS", nativeOutputSha256: digest(output),
    preservedOutputSha256: digest(priorOutput), comparison: "complete worker output" }));
}
