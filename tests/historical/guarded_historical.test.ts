import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { normalizeTranscript, reviewedCorpusTranscript, validateCaptureAdmission } from "./guarded_historical";

const genesis = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const rent = "SysvarRent111111111111111111111111111111111";
const sha = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");
function transcript() {
  // Synthetic parser test only; never used as corpus evidence.
  const raw = '[{"jsonrpc":"2.0","id":3,"result":{"context":{"slot":419704395},"value":{"data":["MBsAAAAAAAAAAAAAAADwPzI=","base64"],"owner":"Sysvar1111111111111111111111111111111111111","executable":false,"lamports":1009200,"rentEpoch":18446744073709551615,"space":17}}}]';
  return { request: [{ jsonrpc: "2.0", id: 3, method: "getAccountInfo", params: [rent,
    { slot: 419704395, encoding: "base64", commitment: "finalized" }] }], httpStatus: 200,
    responseBodyBase64: Buffer.from(raw).toString("base64"), responseSha256: sha(raw) };
}
test("retained transcript keeps exact u64 and target source slot", () => {
  const rows = normalizeTranscript(transcript(), genesis, sha("synthetic-test-only"));
  expect(rows).toHaveLength(1);
  expect(rows[0].value.rentEpoch).toBe("18446744073709551615");
  expect(rows[0].value.sourceSlot).toBe(419704395);
  expect(rows[0].query.phase).toBe("end-slot");
});
test("a parent response cannot be retagged by changing its request", () => {
  const t = transcript(); t.request[0].params[1].slot += 1;
  expect(() => normalizeTranscript(t, genesis, sha("synthetic-test-only"))).toThrow("slot");
});
test("changed response bytes and duplicate response ids reject", () => {
  const t = transcript(); t.responseSha256 = sha("different");
  expect(() => normalizeTranscript(t, genesis, sha("synthetic-test-only"))).toThrow("hash");
  const duplicate = transcript();
  const raw = JSON.parse(Buffer.from(duplicate.responseBodyBase64, "base64").toString());
  const bytes = Buffer.from(JSON.stringify([...raw, ...raw]));
  duplicate.responseBodyBase64 = bytes.toString("base64"); duplicate.responseSha256 = sha(bytes);
  expect(() => normalizeTranscript(duplicate, genesis, sha("synthetic-test-only"))).toThrow("duplicate");
});
test("real-slot controlled transcripts are not admitted as historical observations", () => {
  expect(reviewedCorpusTranscript("/fixtures/m11-controlled-bank-3y2thY/epoch-complete-transcript.json", "/fixtures")).toBe(false);
  expect(reviewedCorpusTranscript("/fixtures/m9-generic-availability-20260906T151300Z/rpc.json", "/fixtures")).toBe(true);
});
test("missing exact rent epoch and malformed account encoding reject", () => {
  for (const mutation of ["missing-u64", "overflow-u64", "wrong-space", "noncanonical-base64"]) {
    const t = transcript();
    let raw = Buffer.from(t.responseBodyBase64, "base64").toString();
    if (mutation === "missing-u64") raw = raw.replace(',"rentEpoch":18446744073709551615', "");
    if (mutation === "overflow-u64") raw = raw.replace("18446744073709551615", "18446744073709551616");
    if (mutation === "wrong-space") raw = raw.replace('"space":17', '"space":18');
    if (mutation === "noncanonical-base64") raw = raw.replace("MBsAAAAAAAAAAAAAAADwPzI=", "MBsAAAAAAAAAAAAAAADwPzI");
    t.responseBodyBase64 = Buffer.from(raw).toString("base64"); t.responseSha256 = sha(raw);
    expect(() => normalizeTranscript(t, genesis, sha("synthetic-test-only"))).toThrow();
  }
});
test("additional capture requires complete pinned native Rent-only observations", () => {
  const entries = Array.from({ length: 16 }, (_, i) => ({ query: { kind: "account", genesisHash: genesis,
    slot: 100 + i, pubkey: rent, phase: "end-slot" }, file: `records/${sha(String(i))}.json`, sha256: sha(String(i)) }));
  const manifest = { schema: "m11-captured-source/v1", identity: { kind: "captured-history", genesisHash: genesis }, entries };
  const receipt = { schema: "svm-replay-m17-exact-rent-capture/v1", status: "COMPLETE", manifestSha256: sha("synthetic-manifest"),
    recordsCaptured: 16, acceptedRentImages: 16, requestedQueries: 16, retries: 0, counters: { requests: 17, account_reads: 16 }, error: null };
  const slots = new Set(entries.map(e => e.query.slot));
  expect(() => validateCaptureAdmission(manifest, receipt.manifestSha256, receipt, slots)).not.toThrow();
  for (const mode of ["partial", "wrong-pin", "non-rent", "outside-targets"]) {
    const m = structuredClone(manifest), r = structuredClone(receipt);
    if (mode === "partial") r.status = "STOPPED_ERROR";
    if (mode === "wrong-pin") r.manifestSha256 = sha("other");
    if (mode === "non-rent") m.entries[0].query.pubkey = "SysvarC1ock11111111111111111111111111111111";
    if (mode === "outside-targets") m.entries[0].query.slot = 999;
    expect(() => validateCaptureAdmission(m, receipt.manifestSha256, r, slots)).toThrow();
  }
});
