import { requiredPath } from "../harness/paths";
/** Migration-only fixture adapter. Rust CLI executes and verifies; no TS coordinator. */
import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
const { rebindOriginalFixture } = await import(join(requiredPath("SVM_REPLAY_TEST_PROTOTYPE"), "poc/m9-historical-regression/provenance"));
const { executorPackageForSource } = await import(join(requiredPath("SVM_REPLAY_TEST_PROTOTYPE"), "poc/m6-prototype-breadth/executor"));
import { frozenPolicy } from "../harness/frozen_policy";

const historical = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m12-sealed-historical-XmOnk2/historical");
const prepared = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m11-historical-final-jWj9Uy");
const buildPath = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m10-sealed-builds-0cEttv/build-manifest.json");
const hash = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");
const read = async (path: string) => JSON.parse(await readFile(path, "utf8"));
const save = async (path: string, data: unknown) => writeFile(path, JSON.stringify(data), { flag: "wx", mode: 0o600 });
const [binaryArg, outputArg, selection = "m7-profile-03"] = process.argv.slice(2);
if (!binaryArg || !outputArg) throw new Error("usage: bun tests/historical/historical.ts BINARY NEW_OUTPUT_DIR [case-id|all|six]");
const buildBinary = resolve(binaryArg), output = resolve(outputArg);
await mkdir(output, { mode: 0o700 });
await mkdir(join(output, "workers"), { mode: 0o700 });
// Cargo may rebuild the developer binary concurrently. Test an immutable copy.
const binary = join(output, "svm-replay");
const builtHash = hash(await readFile(buildBinary));
await copyFile(buildBinary, binary, constants.COPYFILE_EXCL); await chmod(binary, 0o700);
if (hash(await readFile(binary)) !== builtHash) throw new Error("CLI changed during test installation");
const started = performance.now(), startedAt = new Date().toISOString();
const inputsRaw = await readFile(join(historical, "regression-inputs.json"));
const inputs = JSON.parse(inputsRaw.toString());
const buildsRaw = await readFile(buildPath), builds = JSON.parse(buildsRaw.toString()).builds;
const selectedSix = (await read(join(prepared, "summary.json"))).results.map((r: any) => r.caseId);
const rows = inputs.cases.filter((r: any) => selection === "all" || (selection === "six" ? selectedSix.includes(r.candidate.id) : r.candidate.id === selection));
if (rows.length === 0) throw new Error("unknown selection");
const workers = [];
for (const build of builds) {
  const candidate = inputs.cases.find((r: any) => executorPackageForSource(r.candidate.executorSourceId) === build.family)?.candidate;
  if (!candidate || hash(await readFile(build.binary)) !== build.binarySha256) throw new Error("worker missing or changed");
  const file = `workers/${build.family}`;
  await copyFile(build.binary, join(output, file), constants.COPYFILE_EXCL);
  await chmod(join(output, file), 0o700);
  workers.push({ family: build.family, file, sha256: build.binarySha256, buildHash: build.buildHash,
    sourceSha256: build.source.sha256, executorSourceId: candidate.executorSourceId, capabilities: build.capabilities });
}
const catalog = { schema: "svm-replay-workers/v1", platform: { os: "linux", arch: "x86_64", glibcMin: "2.38" }, workers };
const catalogPath = join(output, "catalog.json"); await save(catalogPath, catalog);
const catalogSha256 = hash(await readFile(catalogPath));
const results = [];
for (const row of rows) {
  const caseStarted = performance.now();
  const policy = await frozenPolicy(row);
  const candidate = row.candidate, family = executorPackageForSource(candidate.executorSourceId);
  const build = builds.find((b: any) => b.family === family);
  const blockPath = candidate.blockFile.startsWith("/") ? candidate.blockFile
    : join(join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m4-candidate-discovery-2026-09-03"), candidate.blockFile);
  const rawBlock = await readFile(blockPath);
  if (hash(rawBlock) !== candidate.blockSourceHash) throw new Error("raw historical block changed");
  const fromSourcePreparation = selectedSix.includes(candidate.id);
  const fixturePath = fromSourcePreparation ? join(prepared, candidate.id, "prepared.json") : row.fixturePath;
  const fixtureBytes = await readFile(fixturePath), original = JSON.parse(fixtureBytes.toString());
  if (!fromSourcePreparation && hash(fixtureBytes) !== row.fixtureSha256) throw new Error("frozen fixture changed");
  // Pure identity rebinding, not replay. Never import old execution or verification.
  const fixture = fromSourcePreparation ? original : rebindOriginalFixture(original, build).fixture;
  const requestPath = join(output, `${candidate.id}.json`);
  await save(requestPath, { schema: "svm-replay-prepared/v1", requestId: candidate.id, family, candidate, fixture,
    rawBlockBase64: rawBlock.toString("base64"), blockSha256: candidate.blockSourceHash,
    sourceEvidenceHashes: [hash(fixtureBytes), hash(inputsRaw), hash(buildsRaw), candidate.blockSourceHash, policy.evidenceSha256],
    metadataPolicy: policy.metadataPolicy,
    limits: { timeoutMs: 180_000, maxOutputBytes: 64 * 1024 * 1024, maxDiagnosticBytes: 1024 * 1024 } });
  const child = spawn(binary, ["simulate", "--request", requestPath, "--catalog", catalogPath,
    "--catalog-sha256", catalogSha256, "--data-dir", join(output, "data")], { cwd: output, stdio: ["ignore", "pipe", "pipe"] });
  const stdout: Buffer[] = [], stderr: Buffer[] = [];
  child.stdout.on("data", data => stdout.push(data)); child.stderr.on("data", data => stderr.push(data));
  const code = await new Promise<number | null>((done, fail) => { child.once("error", fail); child.once("close", done); });
  await writeFile(join(output, `${candidate.id}.stdout.json`), Buffer.concat(stdout), { flag: "wx", mode: 0o600 });
  await writeFile(join(output, `${candidate.id}.stderr.txt`), Buffer.concat(stderr), { flag: "wx", mode: 0o600 });
  const receipt = JSON.parse(Buffer.concat(stdout).toString());
  const result = { caseId: candidate.id, family, code, outcome: receipt.outcome, error: receipt.error,
    verification: receipt.verification, elapsedMs: performance.now() - caseStarted,
    fixturePath, fixtureSha256: hash(fixtureBytes), requestSha256: hash(await readFile(requestPath)),
    receiptPath: receipt.receiptPath };
  results.push(result);
  console.log(JSON.stringify({ caseId: result.caseId, code, outcome: result.outcome, error: result.error, elapsedMs: result.elapsedMs }));
  if (hash(await readFile(fixturePath)) !== hash(fixtureBytes) || hash(await readFile(blockPath)) !== candidate.blockSourceHash) throw new Error("source inputs changed during execution");
}
const summary = { schema: "svm-replay-m17-prepared-parity/v1", startedAt, elapsedMs: performance.now() - started,
  status: results.every(r => r.code === 0 && r.outcome === "COMPLETED") ? "PASS" : "FAIL", count: results.length,
  binarySha256: hash(await readFile(binary)), catalogSha256, networkRequests: 0, reconstructedFromSources: false, results };
await save(join(output, "summary.json"), summary);
console.log(JSON.stringify({ status: summary.status, count: summary.count, elapsedMs: summary.elapsedMs }));
if (summary.status !== "PASS") process.exitCode = 1;
