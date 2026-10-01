import { requiredPath } from "../harness/paths";
/** Test-only input construction. Reconstruction and every replay run in native Rust. */
import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { spawn } from "node:child_process";
const { base58Encode } = await import(join(requiredPath("SVM_REPLAY_TEST_PROTOTYPE"), "poc/boundary"));
const { decodeRequestedTransaction } = await import(join(requiredPath("SVM_REPLAY_TEST_PROTOTYPE"), "poc/m9-requested-dependencies"));
const { incrementFinalSystemTransfer } = await import(join(requiredPath("SVM_REPLAY_TEST_PROTOTYPE"), "poc/m12-external/cases"));
import { frozenPolicy } from "../harness/frozen_policy";

const [binaryArg, outArg, catalogRunArg] = process.argv.slice(2);
if (!binaryArg || !outArg || !catalogRunArg) {
  throw new Error("usage: bun tests/overrides/source_variants.ts BINARY NEW_OUTPUT_DIR PREPARED_CATALOG_RUN");
}
const out = resolve(outArg), binary = join(out, "svm-replay"), catalogRun = resolve(catalogRunArg);
const sourceRoot = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m11-historical-final-jWj9Uy/m7-profile-03");
const source = join(sourceRoot, "captured/manifest.json");
const prototype = requiredPath("SVM_REPLAY_TEST_PROTOTYPE");
const digest = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");
const read = async (path: string) => JSON.parse(await readFile(path, "utf8"));
const save = (path: string, value: unknown) => writeFile(path, JSON.stringify(value), { flag: "wx", mode: 0o600 });
const boundFiles = new Map<string, string>();
async function bound(path: string, expected?: string) {
  const bytes = await readFile(path), hash = digest(bytes);
  if (expected !== undefined) assert.equal(hash, expected, `changed pinned input: ${path}`);
  if (boundFiles.has(path)) assert.equal(hash, boundFiles.get(path), `input changed between reads: ${path}`);
  boundFiles.set(path, hash);
  return bytes;
}
const started = performance.now();
await mkdir(out, { mode: 0o700 });
await bound(import.meta.path);
const binaryBytes = await bound(resolve(binaryArg));
await copyFile(resolve(binaryArg), binary, constants.COPYFILE_EXCL);
await chmod(binary, 0o700);
await bound(binary, digest(binaryBytes));
// The original build output may legitimately be rebuilt by another agent later.
boundFiles.delete(resolve(binaryArg));
const catalog = join(catalogRun, "catalog.json");
const catalogPin = (await read(join(catalogRun, "summary.json"))).catalogSha256;
const installed = JSON.parse((await bound(catalog, catalogPin)).toString()).workers;
const sourceBytes = await bound(source), sourcePin = digest(sourceBytes), manifest = JSON.parse(sourceBytes.toString());
const corpusPath = join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m12-sealed-historical-XmOnk2/historical/regression-inputs.json");
const entry = JSON.parse((await bound(corpusPath)).toString()).cases.find((row: any) => row.candidate.id === "m7-profile-03");
assert.ok(entry, "captured case absent from frozen corpus");
const candidate = entry.candidate, policy = await frozenPolicy(entry);
await bound(entry.frozenReceiptPath, policy.evidenceSha256);
const family = installed.find((worker: any) => worker.executorSourceId === candidate.executorSourceId)?.family;
assert.equal(family, "v3-0");
const runtimeBinding = JSON.parse((await bound(join(sourceRoot, "prepared.json"))).toString()).runtime.binding;
async function captured(query: Record<string, unknown>) {
  const rows = manifest.entries.filter((row: any) => Object.entries(query).every(([key, value]) => row.query[key] === value));
  assert.equal(rows.length, 1, `capture must contain one exact query: ${JSON.stringify(query)}`);
  const row = rows[0];
  assert.match(row.file, /^records\/[0-9a-f]{64}\.json$/);
  const record = JSON.parse((await bound(join(dirname(source), row.file), row.sha256)).toString());
  assert.deepEqual(record.query, row.query);
  return record.value;
}
const wire = await captured({ kind: "transaction", slot: candidate.slot, signature: candidate.targetSignature });
assert.equal(typeof wire, "string");
const decoded = decodeRequestedTransaction(wire), message = decoded.transaction.message;
const replacement = incrementFinalSystemTransfer(wire);
const block = await captured({ kind: "block", slot: candidate.slot });
const blockBody = JSON.parse(Buffer.from(block.rawBase64, "base64").toString());
const parentSlot = blockBody.result.parentSlot;
assert.ok(Number.isSafeInteger(parentSlot), "unsafe captured parent slot");
const parent = (pubkey: string) => captured({ kind: "account", slot: parentSlot, pubkey, phase: "end-slot" });
const payer = await parent(message.accountKeys[0]);
assert.equal(payer.presence, "present");
const payerAmount = BigInt(payer.lamports) + (1n << 53n);
assert.ok(payerAmount <= 0xffff_ffff_ffff_ffffn);
const payerOverride = { pubkey: payer.pubkey, lamports: payerAmount.toString() };

// Select from decoded instructions and historical owners, not a fixture-specific program ID.
const loader = "BPFLoaderUpgradeab1e11111111111111111111111";
let program: any, programIndex = -1;
for (let index = 0; index < message.instructions.length; index += 1) {
  const key = message.accountKeys[message.instructions[index].programIdIndex];
  assert.equal(typeof key, "string", "probe requires a static outer program");
  const image = await parent(key);
  if (image.presence === "present" && image.executable && image.owner === loader) {
    program = image; programIndex = index; break;
  }
}
assert.ok(program, "capture lacks an upgradeable outer program for the code override");
const link = Buffer.from(program.dataBase64, "base64");
assert.equal(link.length, 36); assert.equal(link.readUInt32LE(), 2);
const programData = await parent(base58Encode(link.subarray(4)));
assert.equal(programData.presence, "present"); assert.equal(programData.owner, loader);
const oldData = Buffer.from(programData.dataBase64, "base64");
assert.equal(oldData.readUInt32LE(), 3); assert.ok(oldData.length > 45);
const elfManifestPath = join(prototype, "poc/m6-executors/shared/m9-program-overrides/build-manifest.json");
const elfManifestBytes = await bound(elfManifestPath), elfManifest = JSON.parse(elfManifestBytes.toString());
const probe = elfManifest.programs.modified;
for (const file of [...probe.sources.files, ...probe.binary.files]) {
  assert.equal((await bound(file.path, file.sha256)).length, file.bytes);
}
const elf = await bound(probe.elf);
assert.ok(45 + elf.length <= oldData.length, "probe must fit the existing ProgramData allocation");
const changedData = Buffer.alloc(oldData.length);
oldData.copy(changedData, 0, 0, 45); elf.copy(changedData, 45);
assert.deepEqual(changedData.subarray(0, 45), oldData.subarray(0, 45));
const codeOverride = { pubkey: programData.pubkey, dataBase64: changedData.toString("base64") };
const base = { schema: "svm-replay-historical/v1", requestId: "source-variants", family,
  genesisHash: manifest.identity.genesisHash, candidate, runtimeBinding,
  replacementTransactionBase64: null, requestedAccountOverrides: null, bankInputs: [],
  metadataPolicy: policy.metadataPolicy,
  limits: { timeoutMs: 300000, maxOutputBytes: 67108864, maxDiagnosticBytes: 1048576 } };
assert.equal(Object.hasOwn(base, "fixture"), false);
const definitions = [
  { label: "original", request: base },
  { label: "replacement", request: { ...base, replacementTransactionBase64: replacement.transactionBase64 } },
  { label: "account-override", request: { ...base, requestedAccountOverrides: [payerOverride] } },
  { label: "code-override", request: { ...base, requestedAccountOverrides: [codeOverride] } },
  { label: "replacement-account-override", request: { ...base,
    replacementTransactionBase64: replacement.transactionBase64, requestedAccountOverrides: [payerOverride] } },
];
const results: any[] = [], executions = new Map<string, any>();
for (const definition of definitions) {
  const start = performance.now(), requestPath = join(out, `${definition.label}.request.json`);
  await save(requestPath, { ...definition.request, requestId: definition.label });
  const fileTrace = join(out, `${definition.label}.strace`);
  const child = spawn("/usr/bin/strace", ["-f", "--seccomp-bpf", "-e", "trace=%file,%network", "-o", fileTrace,
    binary, "simulate", "--request", requestPath, "--catalog", catalog, "--catalog-sha256", catalogPin,
    "--source", source, "--source-sha256", sourcePin, "--data-dir", join(out, "data")],
    { cwd: out, stdio: ["ignore", "pipe", "pipe"], detached: true });
  const stdout: Buffer[] = [], stderr: Buffer[] = [];
  child.stdout.on("data", bytes => stdout.push(bytes)); child.stderr.on("data", bytes => stderr.push(bytes));
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; if (child.pid) process.kill(-child.pid, "SIGKILL"); }, 330000);
  const code = await new Promise<number | null>((done, fail) => {
    child.once("error", fail); child.once("close", done);
  }).finally(() => clearTimeout(timer));
  const stdoutBytes = Buffer.concat(stdout), stderrBytes = Buffer.concat(stderr);
  await writeFile(join(out, `${definition.label}.stdout.json`), stdoutBytes, { flag: "wx", mode: 0o600 });
  await writeFile(join(out, `${definition.label}.stderr.txt`), stderrBytes, { flag: "wx", mode: 0o600 });
  const trace = await readFile(fileTrace, "utf8"), receipt = JSON.parse(stdoutBytes.toString());
  const row = { label: definition.label, code, timedOut, outcome: receipt.outcome, error: receipt.error,
    controlVerification: receipt.controlVerification?.status, verification: receipt.verification?.status,
    reconstructedFromSources: receipt.reconstructedFromSources, receiptPath: receipt.receiptPath,
    phases: receipt.phases.map((phase: any) => phase.phase),
    prefixCount: receipt.verification?.prefixCount, discoveryAttempts: receipt.preparation?.discoveryAttempts,
    traceSha256: digest(trace), prototypeCheckoutAccess: trace.includes(prototype),
    outgoingInternetSocketObserved: /(?:connect|sendto|sendmsg)\([^\n]*(?:AF_INET|sin_port)/.test(trace),
    elapsedMs: performance.now() - start };
  results.push(row); console.log(JSON.stringify(row));
  if (receipt.output) {
    const outputBytes = await readFile(join(dirname(receipt.receiptPath), receipt.output.file));
    assert.equal(digest(outputBytes), receipt.output.sha256);
    executions.set(definition.label, JSON.parse(outputBytes.toString()));
  }
}
const checks: { name: string; status: string; error?: string }[] = [];
function check(name: string, assertion: () => void) {
  try { assertion(); checks.push({ name, status: "PASS" }); }
  catch (error) { checks.push({ name, status: "FAIL", error: String(error) }); }
}
check("all-native-runs-verified-and-isolated", () => {
  for (const row of results) {
    assert.equal(row.code, 0, JSON.stringify(row)); assert.equal(row.timedOut, false);
    assert.equal(row.outcome, "COMPLETED", JSON.stringify(row));
    assert.equal(row.controlVerification, "PASS"); assert.equal(row.verification, "PASS");
    assert.equal(row.reconstructedFromSources, true);
    assert.equal(row.prototypeCheckoutAccess, false); assert.equal(row.outgoingInternetSocketObserved, false);
    assert.deepEqual(row.phases, row.label === "original" ? ["original-control"] : ["original-control", "requested"]);
  }
});
const original = executions.get("original");
// This prior output is an oracle only: it was not read until all native runs finished.
const baselinePath = join(sourceRoot, "0-verification-output.json");
const baselineBytes = await bound(baselinePath);
check("fresh-original-equals-preserved-historical-output", () => {
  assert.deepEqual(original, JSON.parse(baselineBytes.toString()).output);
});
check("prefix-and-original-control-unchanged-in-every-variant", () => {
  assert.equal(executions.size, definitions.length);
  for (const output of executions.values()) {
    assert.deepEqual(output.prefix, original.prefix); assert.deepEqual(output.original, original.original);
  }
});
function transition(execution: any, pubkey: string) {
  const rows = execution.accountTransitions.filter((row: any) => row.pubkey === pubkey);
  assert.equal(rows.length, 1, `missing/duplicate transition: ${pubkey}`); return rows[0];
}
check("replacement-changes-recipient-by-exact-wire-delta", () => {
  const requested = executions.get("replacement").replacement;
  assert.equal(requested.status, "ok");
  const before = transition(original.original, replacement.proof.recipient).after;
  const after = transition(requested, replacement.proof.recipient).after;
  assert.equal(BigInt(after.lamports) - BigInt(before.lamports), BigInt(replacement.proof.after) - BigInt(replacement.proof.before));
});
check("large-payer-override-applies-after-prefix-with-original-transfer-effect", () => {
  const requested = executions.get("account-override").requestedOverrides;
  assert.equal(requested.status, "ok");
  const before = transition(original.original, payer.pubkey), after = transition(requested, payer.pubkey);
  assert.equal(after.before.lamports, payerOverride.lamports);
  assert.ok(BigInt(after.before.lamports) > (1n << 53n));
  assert.equal(BigInt(after.after.lamports) - BigInt(after.before.lamports), BigInt(before.after.lamports) - BigInt(before.before.lamports));
  assert.deepEqual(transition(requested, replacement.proof.recipient), transition(original.original, replacement.proof.recipient));
});
check("source-built-code-override-executes-error-43-at-selected-program", () => {
  const requested = executions.get("code-override").requestedOverrides;
  assert.equal(requested.status, "err");
  // This is the public source-built program's instruction, not an archived replay answer.
  assert.deepEqual(JSON.parse(requested.normalizedError), { InstructionError: [programIndex, { Custom: 43 }] });
  assert.ok(requested.logs.some((line: string) => line === `Program ${program.pubkey} failed: custom program error: 0x2b`));
  assert.notDeepEqual(requested, original.original);
});
check("combined-payload-and-account-override-retain-both-independent-effects", () => {
  const requested = executions.get("replacement-account-override").requestedOverrides;
  assert.equal(requested.status, "ok");
  const before = transition(original.original, payer.pubkey), after = transition(requested, payer.pubkey);
  const delta = BigInt(replacement.proof.after) - BigInt(replacement.proof.before);
  assert.equal(after.before.lamports, payerOverride.lamports);
  assert.equal(BigInt(after.after.lamports) - BigInt(after.before.lamports),
    BigInt(before.after.lamports) - BigInt(before.before.lamports) - delta);
  const receiver = transition(requested, replacement.proof.recipient).after;
  assert.equal(BigInt(receiver.lamports) - BigInt(transition(original.original, replacement.proof.recipient).after.lamports), delta);
});
for (const [path, hash] of boundFiles) await bound(path, hash);
const summary = { schema: "svm-replay-m17-source-variants/v1", status: checks.every(row => row.status === "PASS") ? "PASS" : "FAIL",
  caseId: candidate.id, family, metadataPolicy: policy.metadataPolicy, policyEvidenceSha256: policy.evidenceSha256,
  sourcePin, catalogPin, binarySha256: digest(binaryBytes), baselinePath, baselineSha256: digest(baselineBytes),
  replacementProof: replacement.proof, payerOverride,
  codeProof: { program: program.pubkey, instructionIndex: programIndex, programData: programData.pubkey,
    manifestSha256: digest(elfManifestBytes), elfSha256: digest(elf), headerSha256: digest(oldData.subarray(0, 45)),
    originalDataSha256: digest(oldData), overriddenDataSha256: digest(changedData), allocatedBytes: changedData.length },
  boundInputs: Object.fromEntries(boundFiles), boundInputsUnchanged: true, results, checks,
  networkScope: "captured disk source; strace inspected native CLI and worker network syscalls; no live provider RPC test",
  elapsedMs: performance.now() - started };
await save(join(out, "summary.json"), summary);
console.log(JSON.stringify({ status: summary.status, checks, elapsedMs: summary.elapsedMs }));
if (summary.status !== "PASS") process.exitCode = 1;
