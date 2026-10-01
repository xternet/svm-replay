import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { checkedFile } from "./clean_host.mjs";

test("clean-host inputs require a separately supplied matching SHA-256", async () => {
    const root = await mkdtemp(join(tmpdir(), "svm-release-input-"));
    try {
        const path = join(root, "input");
        const bytes = Buffer.from("release input");
        await writeFile(path, bytes);
        await assert.rejects(checkedFile(path, "invalid"), /SHA-256/);
        await assert.rejects(checkedFile(path, "0".repeat(64)), /hash mismatch/);
        assert.deepEqual(await checkedFile(path, createHash("sha256").update(bytes).digest("hex")), bytes);
    } finally {
        await rm(root, { recursive: true });
    }
});

test("clean-host check refuses an incomplete invocation", () => {
    const result = spawnSync(process.execPath, [new URL("./clean_host.mjs", import.meta.url).pathname], { encoding: "utf8" });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /usage:/);
});
