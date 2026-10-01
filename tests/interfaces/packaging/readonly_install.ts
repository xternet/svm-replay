/** Linux installation/layout proof using an already verified prepared boundary. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { chmod, copyFile, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { spawn } from "node:child_process";

const [binaryArg, outArg, catalogRunArg, receiptArg] = process.argv.slice(2);
if (!binaryArg || !outArg || !catalogRunArg || !receiptArg) {
  throw new Error("usage: bun tests/interfaces/packaging/readonly_install.ts BINARY NEW_OUT CATALOG_RUN SUCCESS_RECEIPT");
}
const digest = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const read = async (file: string) => JSON.parse(await readFile(file, "utf8"));
const save = (file: string, value: unknown) => writeFile(file, JSON.stringify(value), { flag: "wx", mode: 0o600 });
const out = resolve(outArg), install = join(out, "install"), catalogRun = resolve(catalogRunArg);
await mkdir(out, { mode: 0o700 }); await mkdir(install, { mode: 0o700 });
const prior = await read(resolve(receiptArg)); assert.equal(prior.outcome, "COMPLETED");
const requestBytes = await readFile(join(dirname(prior.receiptPath), prior.preparedRequest.file));
assert.equal(digest(requestBytes), prior.preparedRequest.sha256);
const request = JSON.parse(requestBytes.toString());
const originalCatalog = await readFile(join(catalogRun, "catalog.json"));
assert.equal(digest(originalCatalog), (await read(join(catalogRun, "summary.json"))).catalogSha256);
const catalog = JSON.parse(originalCatalog.toString());
const selected = catalog.workers.find((worker: any) => worker.family === request.family);
assert.ok(selected);
const binaryBytes = await readFile(resolve(binaryArg));
const workerBytes = await readFile(join(catalogRun, selected.file));
assert.equal(digest(workerBytes), selected.sha256);
await copyFile(resolve(binaryArg), join(install, "svm-replay"), constants.COPYFILE_EXCL);
await copyFile(join(catalogRun, selected.file), join(install, "worker"), constants.COPYFILE_EXCL);
await writeFile(join(out, "request.json"), requestBytes, { flag: "wx", mode: 0o400 });
const installedCatalog = { ...catalog, workers: [{ ...selected, file: "worker" }] };
await save(join(install, "catalog.json"), installedCatalog);
const catalogBytes = await readFile(join(install, "catalog.json"));
const pins = { "svm-replay": digest(binaryBytes), worker: selected.sha256, "catalog.json": digest(catalogBytes) };
for (const [file, pin] of Object.entries(pins)) {
  assert.equal(digest(await readFile(join(install, file))), pin);
  await chmod(join(install, file), file === "catalog.json" ? 0o400 : 0o500);
}
await chmod(install, 0o500);
const started = performance.now();
const child = spawn("strace", ["-ff", "-e", "trace=file,process,network", "-o", join(out, "syscalls"),
  join(install, "svm-replay"), "simulate", "--request", join(out, "request.json"),
  "--catalog", join(install, "catalog.json"), "--catalog-sha256", pins["catalog.json"]],
  { cwd: out, env: { PATH: process.env.PATH, XDG_DATA_HOME: join(out, "user-data") }, stdio: ["ignore", "pipe", "pipe"] });
const stdout: Buffer[] = [], stderr: Buffer[] = [];
child.stdout.on("data", value => stdout.push(value)); child.stderr.on("data", value => stderr.push(value));
const code = await new Promise<number | null>((done, fail) => { child.once("error", fail); child.once("close", done); });
await writeFile(join(out, "stdout.json"), Buffer.concat(stdout), { flag: "wx", mode: 0o600 });
await writeFile(join(out, "stderr.txt"), Buffer.concat(stderr), { flag: "wx", mode: 0o600 });
const receipt = JSON.parse(Buffer.concat(stdout).toString());
assert.equal(code, 0); assert.equal(receipt.outcome, "COMPLETED");
assert.ok(receipt.receiptPath.startsWith(join(out, "user-data", "svm-replay", "runs") + "/"));
assert.deepEqual((await readdir(install)).sort(), Object.keys(pins).sort());
for (const [file, pin] of Object.entries(pins)) {
  assert.equal(digest(await readFile(join(install, file))), pin);
  assert.equal((await stat(join(install, file))).mode & 0o222, 0);
}
const logs = (await readdir(out)).filter(file => file.startsWith("syscalls."));
assert.ok(logs.length > 0);
for (const file of logs) {
  const text = await readFile(join(out, file), "utf8");
  assert.ok(!text.includes("/grant0/"), "runtime accessed prototype checkout");
  assert.ok(!/connect\(.*AF_INET/.test(text), "runtime attempted IP networking");
}
const summary = { status: "PASS", caseId: request.candidate.id, receiptPath: receipt.receiptPath,
  binarySha256: pins["svm-replay"], workerSha256: pins.worker, catalogSha256: pins["catalog.json"],
  readonlyInstallUnchanged: true, explicitDataDirectory: false, defaultDataRoot: join(out, "user-data", "svm-replay"),
  noPrototypeFileAccess: true, noIpConnectCalls: true, syscallLogs: logs.length, elapsedMs: performance.now() - started };
await save(join(out, "summary.json"), summary); console.log(JSON.stringify(summary));
