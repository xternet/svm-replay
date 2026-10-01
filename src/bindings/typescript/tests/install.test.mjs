import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { selectPlatform, inspectRelease, validateArchivePaths } from "../install.mjs";

test("installer selects only native qualified OS/CPU pairs", () => {
    assert.equal(selectPlatform("linux", "x64"), "linux-x86_64");
    assert.equal(selectPlatform("darwin", "arm64"), "macos-aarch64");
    assert.equal(selectPlatform("win32", "arm64"), "windows-aarch64");
    assert.throws(() => selectPlatform("linux", "ia32"), /unsupported/);
});

test("archive paths reject traversal, alternate roots and ambiguous names", () => {
    validateArchivePaths("bundle/\nbundle/bin/svm-replay\nLICENSE\nruntime-sources/Cargo.toml\n");
    validateArchivePaths("bundle/\r\nbundle/bin/svm-replay.exe\r\nLICENSE\r\n");
    for (const entry of ["../escape", "/absolute", "bundle/../escape", "bundle/a\\b", "bundle/a b", "another/file", "bundle/C:evil"]) {
        assert.throws(() => validateArchivePaths(entry + "\n"), /unsafe/);
    }
});

test("release pin is mandatory and platform metadata is confined", async () => {
    const dir = await mkdtemp(join(tmpdir(), "svm-install-test-"));
    try {
        const path = join(dir, "release.json");
        const raw = JSON.stringify({version:"0.1.0", platforms:[{platform:"linux-x86_64",file:"native.tar.gz",sha256:"1".repeat(64),bundleSha256:"2".repeat(64)}]});
        await writeFile(path, raw);
        await assert.rejects(inspectRelease(path, "0".repeat(64), "linux-x86_64"), /integrity/);
        const pin = createHash("sha256").update(raw).digest("hex");
        assert.equal((await inspectRelease(path, pin, "linux-x86_64")).file, "native.tar.gz");
        await assert.rejects(inspectRelease(path, pin, "windows-x86_64"), /not included/);
    } finally { await rm(dir, {recursive:true}); }
});
