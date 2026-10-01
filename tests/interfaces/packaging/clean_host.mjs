/** Portable release check. Needs trusted candidate pins and a reviewed prepared request. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { arch, platform, release } from "node:os";

const digest = bytes => createHash("sha256").update(bytes).digest("hex");
export async function checkedFile(path, pin) {
    assert.match(pin, /^[a-f0-9]{64}$/, "expected a SHA-256 pin");
    assert.ok((await lstat(path)).isFile(), `not a regular file: ${path}`);
    const bytes = await readFile(path);
    assert.equal(digest(bytes), pin, `hash mismatch: ${path}`);
    return bytes;
}
function confined(root, path) {
    assert.equal(isAbsolute(path), false, "expected a relative artifact path");
    const full = resolve(root, path), rel = relative(root, full);
    assert.ok(rel !== ".." && !rel.startsWith(`..${sep}`), "artifact escapes its directory");
    return full;
}
async function main(args) {
    assert.equal(args.length, 6, "usage: node clean_host.mjs BUNDLE_JSON BUNDLE_SHA256 REQUEST_JSON REQUEST_SHA256 EXPECTED_OUTPUT_SHA256 NEW_OUTPUT_DIRECTORY");
    const [bundleArg, bundlePin, requestArg, requestPin, outputPin, outArg] = args;
    assert.match(outputPin, /^[a-f0-9]{64}$/);
    const bundlePath = resolve(bundleArg), requestPath = resolve(requestArg), out = resolve(outArg);
    const bundle = JSON.parse(await checkedFile(bundlePath, bundlePin));
    assert.equal(bundle.schema, "svm-replay-bundle/v1");
    const entry = bundle.files.find(file => file.path === bundle.binary);
    assert.ok(entry, "bundle has no coordinator identity");
    const bootstrap = confined(dirname(bundlePath), bundle.binary);
    await checkedFile(bootstrap, entry.sha256);
    const request = JSON.parse(await checkedFile(requestPath, requestPin));
    assert.equal(request.schema, "svm-replay-prepared/v1");
    await mkdir(out, { mode: 0o700 }); // Existing evidence must never be overwritten.
    const install = join(out, "install"), data = join(out, "data");
    const started = Date.now();
    async function run(name, binary, argv) {
        const result = spawnSync(binary, argv, {
            cwd: out, env: { PATH: process.env.PATH, LANG: "C.UTF-8" },
            encoding: "utf8", timeout: 300_000, maxBuffer: 32 * 1024 * 1024,
        });
        if (result.error) {
            await writeFile(join(out, `${name}.error`), String(result.error), { flag: "wx" });
            throw result.error;
        }
        assert.equal(typeof result.stdout, "string");
        assert.equal(typeof result.stderr, "string");
        await writeFile(join(out, `${name}.stdout`), result.stdout, { flag: "wx" });
        await writeFile(join(out, `${name}.stderr`), result.stderr, { flag: "wx" });
        assert.equal(result.status, 0, `${name} failed: ${result.stderr}`);
        return JSON.parse(result.stdout);
    }
    await run("install", bootstrap, ["bundle", "install", "--bundle", bundlePath,
        "--bundle-sha256", bundlePin, "--output", install]);
    const installedManifest = join(install, "bundle.json");
    await checkedFile(installedManifest, bundlePin);
    const binary = confined(install, bundle.binary);
    await checkedFile(binary, entry.sha256);
    await run("doctor", binary, ["doctor", "--bundle", installedManifest, "--bundle-sha256", bundlePin]);
    const receipt = await run("replay", binary, ["simulate", "--bundle", installedManifest,
        "--bundle-sha256", bundlePin, "--request", requestPath, "--data-dir", data]);
    assert.equal(receipt.outcome, "COMPLETED");
    assert.equal(receipt.currentStateFallback, false);
    assert.equal(receipt.implementationSha256, entry.sha256);
    assert.equal(receipt.processOwnerSha256, entry.sha256);
    assert.equal(receipt.controlVerification.status, "PASS");
    assert.equal(receipt.verification.status, "PASS");
    assert.ok(receipt.receiptPath.startsWith(`${data}${sep}`));
    const saved = JSON.parse(await readFile(receipt.receiptPath));
    const { receiptPath, ...envelope } = receipt; // CLI adds the path; stored receipt is portable.
    assert.deepEqual(saved, envelope);
    const outputPath = confined(dirname(receipt.receiptPath), receipt.output.file);
    await checkedFile(outputPath, receipt.output.sha256);
    assert.equal(receipt.output.sha256, outputPin, "differs from the reviewed reference result");
    await checkedFile(requestPath, requestPin);
    await checkedFile(bundlePath, bundlePin);
    const summary = { status: "PASS", scope: "One real prepared replay and native installation; not full corpus or provider acquisition",
        host: { os: platform(), arch: arch(), kernel: release(), node: process.version },
        bundleSha256: bundlePin, requestSha256: requestPin, outputSha256: outputPin,
        binarySha256: entry.sha256, caseId: request.candidate.id, elapsedMs: Date.now() - started,
        receiptPath: receipt.receiptPath };
    await writeFile(join(out, "summary.json"), JSON.stringify(summary, null, 2), { flag: "wx" });
    console.log(JSON.stringify(summary));
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    await main(process.argv.slice(2));
}
