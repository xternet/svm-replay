import { requiredPath } from "../harness/paths";
/** Offline prepared replay with separately retained target-slot observations.
 * No prototype coordinator, acquisition, fixture retagging, or expected outputs.
 */
import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { parse, isLosslessNumber } from "lossless-json";
import { frozenPolicy } from "../harness/frozen_policy";
import { checkSourceReceipt } from "../harness/receipt_artifacts";

const genesis = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const generic = new Set(["SysvarC1ock11111111111111111111111111111111",
  "SysvarEpochSchedu1e111111111111111111111111", "SysvarEpochRewards1111111111111111111111111",
  "SysvarRent111111111111111111111111111111111", "SysvarS1otHashes111111111111111111111111111",
  "SysvarStakeHistory1111111111111111111111111", "SysvarLastRestartS1ot1111111111111111111111"]);
const hash = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");
const read = async (path: string) => JSON.parse(await readFile(path, "utf8"));
const save = async (path: string, value: unknown) => writeFile(path, JSON.stringify(value), { flag: "wx", mode: 0o600 });
/** Explicit reviewed provenance, not a claim based on the transcript's shape. */
export function reviewedCorpusTranscript(path: string, artifacts: string): boolean {
  return [join(artifacts, "m9-generic-availability-20260906T151300Z/rpc.json"),
    join(artifacts, "m9-reward-discovery-20260906T150800Z-attempt2/rpc-001.json")].includes(path);
}
export function validateCaptureAdmission(manifest: any, pin: string, receipt: any, slots: Set<number>): void {
  require(manifest.schema === "m11-captured-source/v1" && manifest.identity.kind === "captured-history"
    && manifest.identity.genesisHash === genesis, "additional capture identity differs");
  require(receipt.schema === "svm-replay-m17-exact-rent-capture/v1" && receipt.status === "COMPLETE" && receipt.manifestSha256 === pin
    && receipt.recordsCaptured === 16 && receipt.acceptedRentImages === 16 && receipt.requestedQueries === 16 && receipt.retries === 0
    && receipt.counters.requests === 17 && receipt.counters.account_reads === 16 && receipt.error === null, "additional capture is partial or unpinned");
  require(Array.isArray(manifest.entries) && manifest.entries.length === 16, "additional capture record count differs");
  const seen = new Set<number>();
  for (const { query, file, sha256 } of manifest.entries) {
    require(query.kind === "account" && query.genesisHash === genesis && query.phase === "end-slot"
      && query.pubkey === "SysvarRent111111111111111111111111111111111" && Object.keys(query).length === 5
      && Number.isSafeInteger(query.slot) && slots.has(query.slot) && !seen.has(query.slot), "additional capture is not unique target Rent");
    require(/^[a-f0-9]{64}$/.test(sha256) && file === `records/${sha256}.json`, "additional capture record path differs");
    seen.add(query.slot);
  }
}
function require(ok: unknown, message: string): asserts ok { if (!ok) throw new Error(message); }
function decimal(value: unknown): string {
  const text = isLosslessNumber(value) ? value.value : typeof value === "number" && Number.isSafeInteger(value) ? String(value) : "";
  require(/^(0|[1-9][0-9]*)$/.test(text) && BigInt(text) <= 18446744073709551615n, "noncanonical u64");
  return text;
}
function integer(value: unknown): number {
  const parsed = Number(decimal(value)); require(Number.isSafeInteger(parsed), "unsafe integer"); return parsed;
}
function base64(value: unknown): Buffer {
  require(typeof value === "string", "base64 string required");
  const bytes = Buffer.from(value, "base64"); require(bytes.toString("base64") === value, "noncanonical base64"); return bytes;
}

/** Validate genuine request/response bindings before producing normalized images. */
export function normalizeTranscript(transcript: any, network: string, containerHash: string): any[] {
  require(transcript.httpStatus === undefined || transcript.httpStatus === 200, "unsuccessful HTTP status");
  const raw = base64(transcript.responseBodyBase64);
  require(hash(raw) === transcript.responseSha256, "raw response hash differs");
  for (const field of ["responseBytes", "bytes"]) {
    if (transcript[field] !== undefined) require(transcript[field] === raw.length, "response length differs");
  }
  const requests = Array.isArray(transcript.request) ? transcript.request : [transcript.request];
  const parsed: any = parse(raw.toString("utf8"));
  const responses = Array.isArray(parsed) ? parsed : [parsed];
  const byId = new Map<number, any>();
  for (const response of responses) {
    const id = integer(response.id);
    require(!byId.has(id), "duplicate response id"); require(response.jsonrpc === "2.0", "response JSON-RPC version");
    byId.set(id, response);
  }
  const seen = new Set<number>(), records = [];
  for (const request of requests) {
    require(request.jsonrpc === "2.0", "request JSON-RPC version");
    const id = integer(request.id); require(!seen.has(id), "duplicate request id"); seen.add(id);
    const response = byId.get(id); require(response !== undefined, "missing response id");
    if (request.method !== "getAccountInfo" || !generic.has(request.params[0])) continue;
    require(!Object.hasOwn(response, "error") && Object.hasOwn(response, "result"), "RPC error or missing result");
    const [key, options] = request.params;
    require(options.encoding === "base64" && options.commitment === "finalized", "historical query configuration");
    const slot = integer(options.slot);
    require(integer(response.result.context.slot) === slot, "response slot differs from requested slot");
    const query = { kind: "account", genesisHash: network, slot, pubkey: key, phase: "end-slot" };
    const source = response.result.value;
    let value: any = { pubkey: key, sourceSlot: slot, role: "sysvar", presence: source === null ? "absent" : "present" };
    if (source !== null) {
      require(source.owner === "Sysvar1111111111111111111111111111111111111" && source.executable === false, "not a nonexecutable sysvar");
      require(Array.isArray(source.data) && source.data.length === 2 && source.data[1] === "base64", "account encoding");
      const bytes = base64(source.data[0]);
      if (source.space !== undefined) require(integer(source.space) === bytes.length, "account space differs");
      value = { ...value, lamports: decimal(source.lamports), owner: source.owner, executable: source.executable,
        rentEpoch: decimal(source.rentEpoch), dataBase64: source.data[0] };
    }
    records.push({ query, value, evidenceHashes: [containerHash, transcript.responseSha256] });
  }
  require(byId.size === seen.size, "unexpected response id");
  return records;
}

async function exportRetained(output: string, slots: Set<number>, additional?: { path: string; pin: string }) {
  const artifacts = requiredPath("SVM_REPLAY_TEST_ARTIFACTS");
  const paths = execFileSync("rg", ["--files", "-uuu", artifacts], { maxBuffer: 128 * 1024 * 1024 }).toString().trim().split("\n").sort();
  const rpcPaths = paths.filter(path => /\/(rpc[^/]*|[^/]*transcript[^/]*)\.json$/.test(path));
  const manifestPaths = paths.filter(path => path.endsWith("/captured/manifest.json"));
  const blobPaths = paths.filter(path => /\/(cache|store)\/blobs\//.test(path));
  const sourceRoot = join(output, "retained-source");
  await mkdir(sourceRoot); await mkdir(join(sourceRoot, "records")); await mkdir(join(sourceRoot, "raw"));
  const records = new Map<string, any>(), evidence = [], unadmittedCandidates = [], copied = new Set<string>();
  const retainedRentSlots = new Set<number>();
  for (const path of manifestPaths) {
    const manifest = await read(path);
    for (const entry of manifest.entries) if (entry.query.kind === "account" && entry.query.pubkey === "SysvarRent111111111111111111111111111111111") retainedRentSlots.add(entry.query.slot);
  }
  const cacheRentSlots = new Set<number>();
  for (const path of blobPaths) {
    const raw = await readFile(path);
    require(hash(raw) === path.split("/").at(-1), "cache blob filename hash differs");
    if (!raw.includes("SysvarRent111111111111111111111111111111111")) continue;
    const value = JSON.parse(raw.toString());
    if (value.schema === "historical-rpc-cache/v1" && value.query.method === "getAccountInfo"
      && value.query.params[0] === "SysvarRent111111111111111111111111111111111") cacheRentSlots.add(value.query.expectedSlot);
  }
  for (const path of rpcPaths) {
    const raw = await readFile(path), value = JSON.parse(raw.toString()), containerHash = hash(raw);
    for (const transcript of Array.isArray(value) ? value : [value]) {
      const requests = Array.isArray(transcript.request) ? transcript.request : [transcript.request];
      if (!requests.some((q: any) => q?.method === "getAccountInfo" && generic.has(q.params[0]) && slots.has(q.params[1].slot))) continue;
      if (!reviewedCorpusTranscript(path, artifacts)) { unadmittedCandidates.push({ path, sha256: containerHash }); continue; }
      const normalized = normalizeTranscript(transcript, genesis, containerHash);
      for (const record of normalized.filter(row => slots.has(row.query.slot))) {
        const key = `${record.query.slot}:${record.query.pubkey}`, prior = records.get(key);
        require(prior === undefined || JSON.stringify(prior.value) === JSON.stringify(record.value), "conflicting retained observations");
        if (prior !== undefined) continue;
        records.set(key, record);
        evidence.push({ query: record.query, originPath: path, originSha256: containerHash,
          rawResponseSha256: transcript.responseSha256, originalNetworkCallsThisRun: 0 });
      }
      for (const [digest, bytes] of [[containerHash, raw], [transcript.responseSha256, base64(transcript.responseBodyBase64)]] as [string, Buffer][]) {
        if (!copied.has(digest)) { await writeFile(join(sourceRoot, "raw", digest), bytes, { flag: "wx", mode: 0o600 }); copied.add(digest); }
      }
    }
  }
  require(records.size > 0, "no exact retained generic observations found");
  const importedCaptures = [];
  if (additional) {
    const manifestBytes = await readFile(additional.path); require(hash(manifestBytes) === additional.pin, "additional manifest digest differs");
    const manifest = JSON.parse(manifestBytes.toString()), captureRoot = dirname(additional.path);
    const receiptBytes = await readFile(join(captureRoot, "capture-receipt.json")), receipt = JSON.parse(receiptBytes.toString());
    validateCaptureAdmission(manifest, additional.pin, receipt, slots);
    for (const entry of manifest.entries) {
      const bytes = await readFile(join(captureRoot, entry.file)); require(hash(bytes) === entry.sha256, "additional record digest differs");
      const record = JSON.parse(bytes.toString());
      require(JSON.stringify(Object.entries(record.query).sort()) === JSON.stringify(Object.entries(entry.query).sort()), "additional record query differs");
      require(record.provenance?.schema === "m17-alchemy-rpc-provenance/v1", "additional record lacks native raw provenance");
      // Native CapturedSource validates request, response, genesis and exact u64
      // normalization again before these records can hydrate execution.
      const key = `${record.query.slot}:${record.query.pubkey}`; require(!records.has(key), "additional capture conflicts with retained query");
      records.set(key, record);
      evidence.push({ query: record.query, originPath: join(captureRoot, entry.file), originSha256: entry.sha256,
        originManifestSha256: additional.pin, provenanceKind: record.provenance.schema, rawResponseSha256: record.provenance.responseSha256,
        originalNetworkCallsThisRun: 0 });
    }
    await writeFile(join(sourceRoot, "raw", additional.pin), manifestBytes, { flag: "wx", mode: 0o600 });
    const receiptHash = hash(receiptBytes); await writeFile(join(sourceRoot, "raw", receiptHash), receiptBytes, { flag: "wx", mode: 0o600 });
    importedCaptures.push({ manifestPath: additional.path, manifestSha256: additional.pin, receiptSha256: receiptHash, identity: manifest.identity });
  }
  const entries = [];
  for (const [, record] of [...records].sort(([a], [b]) => a.localeCompare(b))) {
    const bytes = JSON.stringify(record), digest = hash(bytes), file = `records/${digest}.json`;
    await writeFile(join(sourceRoot, file), bytes, { flag: "wx", mode: 0o600 }); entries.push({ query: record.query, file, sha256: digest });
  }
  const observedSlots = entries.map(e => e.query.slot);
  const manifest = { schema: "m11-captured-source/v1", identity: { id: "m17-retained-generic-observations", version: "v1", genesisHash: genesis,
    kind: "captured-history", coverage: { firstSlot: Math.min(...observedSlots), lastSlot: Math.max(...observedSlots), completeness: "partial" }, capabilities: ["account"] }, entries };
  const manifestPath = join(sourceRoot, "manifest.json"); await save(manifestPath, manifest);
  const inventory = { schema: "svm-replay-m17-retained-generic-inventory/v1", scope: "retained-rpc-transcripts-captured-manifests-and-cache-blobs",
    networkRequests: 0, fixtureImagesUsedAsSourceRecords: false, expectedOutputsRead: false, rpcFilesScanned: rpcPaths.length,
    capturedManifestsScanned: manifestPaths.length, cacheBlobsScanned: blobPaths.length,
    capturedRentSlots: [...retainedRentSlots].sort(), cacheRentSlots: [...cacheRentSlots].sort(), evidence, unadmittedCandidates, importedCaptures };
  await save(join(sourceRoot, "inventory.json"), inventory);
  return { manifestPath, manifestSha256: hash(await readFile(manifestPath)), inventory,
    importedEvidenceHashes: importedCaptures.flatMap(item => [item.manifestSha256, item.receiptSha256]) };
}

async function main() {
  const historical = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m12-sealed-historical-XmOnk2/historical/regression-inputs.json");
  const [binaryArg, baseArg, outputArg, selection = "all", additionalPath, additionalPin] = process.argv.slice(2);
  require(binaryArg && baseArg && outputArg, "usage: bun tests/historical/guarded_historical.ts BINARY PREVIOUS_PREPARED_RUN NEW_OUTPUT_DIR [all|case-id|inventory] [CAPTURE_MANIFEST CAPTURE_SHA256]");
  require((additionalPath === undefined) === (additionalPin === undefined), "additional manifest and pin must be paired");
  const base = resolve(baseArg), output = resolve(outputArg), sourceBinary = resolve(binaryArg);
  await mkdir(output, { mode: 0o700 });
  const baseline = await read(join(base, "summary.json")), historicalBytes = await readFile(historical), rows = JSON.parse(historicalBytes.toString()).cases;
  const requests = new Map<string, any>();
  for (const row of baseline.results) {
    const bytes = await readFile(join(base, `${row.caseId}.json`));
    require(hash(bytes) === row.requestSha256, "previous request changed"); requests.set(row.caseId, JSON.parse(bytes.toString()));
  }
  const capture = await exportRetained(output, new Set([...requests.values()].map(r => r.fixture.target.targetSlot)),
    additionalPath === undefined ? undefined : { path: resolve(additionalPath), pin: additionalPin! });
  console.log(JSON.stringify({ event: "retained-source-ready", ...capture, inventory: { records: capture.inventory.evidence.length } }));
  if (selection === "inventory") return;
  const started = performance.now(), binary = join(output, "svm-replay"), binaryHash = hash(await readFile(sourceBinary));
  await copyFile(sourceBinary, binary, constants.COPYFILE_EXCL); await chmod(binary, 0o700);
  require(hash(await readFile(binary)) === binaryHash, "CLI changed during copy");
  const catalogBytes = await readFile(join(base, "catalog.json")); require(hash(catalogBytes) === baseline.catalogSha256, "catalog changed");
  const catalog = JSON.parse(catalogBytes.toString()); await mkdir(join(output, "workers"));
  for (const worker of catalog.workers) {
    const from = join(base, worker.file), to = join(output, worker.file);
    require(hash(await readFile(from)) === worker.sha256, "worker identity changed");
    await copyFile(from, to, constants.COPYFILE_EXCL); await chmod(to, 0o700);
    require(hash(await readFile(to)) === worker.sha256, "worker changed during copy");
  }
  const catalogPath = join(output, "catalog.json"); await writeFile(catalogPath, catalogBytes, { flag: "wx", mode: 0o600 });
  const results = [], policies = [];
  for (const row of rows) policies.push({ caseId: row.candidate.id, ...await frozenPolicy(row) });
  require(policies.filter(p => p.metadataPolicy === "STRICT").length === 41 && policies.filter(p => p.metadataPolicy !== "STRICT").length === 6, "frozen corpus admission count differs");
  await save(join(output, "frozen-admissions.json"), policies);
  for (const [caseId, original] of requests) {
    if (selection !== "all" && selection !== caseId) continue;
    const policy = policies.find(p => p.caseId === caseId); require(policy, "case has no frozen admission");
    const request = { ...original, metadataPolicy: policy.metadataPolicy,
      sourceEvidenceHashes: [...new Set([...original.sourceEvidenceHashes, policy.evidenceSha256, hash(historicalBytes), capture.manifestSha256, ...capture.importedEvidenceHashes])] };
    const requestPath = join(output, `${caseId}.json`); await save(requestPath, request);
    const child = spawn(binary, ["simulate", "--request", requestPath, "--catalog", catalogPath, "--catalog-sha256", baseline.catalogSha256,
      "--source", capture.manifestPath, "--source-sha256", capture.manifestSha256, "--genesis-hash", genesis, "--data-dir", join(output, "data")],
      { cwd: output, stdio: ["ignore", "pipe", "pipe"] });
    const stdout: Buffer[] = [], stderr: Buffer[] = [];
    child.stdout.on("data", data => stdout.push(data)); child.stderr.on("data", data => stderr.push(data));
    const code = await new Promise<number | null>((done, fail) => { child.once("error", fail); child.once("close", done); });
    await writeFile(join(output, `${caseId}.stdout.json`), Buffer.concat(stdout), { flag: "wx", mode: 0o600 });
    await writeFile(join(output, `${caseId}.stderr.txt`), Buffer.concat(stderr), { flag: "wx", mode: 0o600 });
    const receipt = JSON.parse(Buffer.concat(stdout).toString());
    await checkSourceReceipt(receipt);
    require(receipt.reconstructedFromSources === false && receipt.scope === "supplied-prepared-replay/v1", "prepared replay mislabeled as reconstruction");
    const result = { caseId, code, outcome: receipt.outcome, error: receipt.error, metadataPolicy: policy.metadataPolicy,
      verification: receipt.verification, preparation: receipt.preparation, receiptPath: receipt.receiptPath, requestSha256: hash(await readFile(requestPath)) };
    results.push(result); console.log(JSON.stringify({ caseId, code, outcome: receipt.outcome, error: receipt.error }));
  }
  require(results.length > 0, "unknown selection");
  const summary = { schema: "svm-replay-m17-guarded-prepared-parity/v1", status: results.every(r => r.code === 0 && r.outcome === "COMPLETED") ? "PASS" : "INCOMPLETE",
    binarySha256: binaryHash, catalogSha256: baseline.catalogSha256, manifestSha256: capture.manifestSha256,
    count: results.length, elapsedMs: performance.now() - started, networkRequests: 0, reconstructedFromSources: false,
    sourceHydrationEnabled: true, completed: results.filter(r => r.outcome === "COMPLETED").length, results };
  await save(join(output, "summary.json"), summary); console.log(JSON.stringify({ status: summary.status, completed: summary.completed, count: summary.count, elapsedMs: summary.elapsedMs }));
  if (summary.status !== "PASS") process.exitCode = 1;
}
if (import.meta.main) await main();
