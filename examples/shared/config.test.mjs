import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadConfig } from "./config.mjs";

test("example paths are relative to their config, not the calling shell", async () => {
    const directory = await mkdtemp(join(tmpdir(), "svm-replay-config-"));
    try {
        const path = join(directory, "replay.json");
        await writeFile(path, JSON.stringify({requestPath: "request.json", installation: {
            bundlePath: "bundle.json", dataDir: "data", source: {path: "source.json", sha256: "pin"},
            alchemyConfig: "alchemy.json",
        }}));
        const config = await loadConfig(path);
        assert.equal(config.requestPath, join(directory, "request.json"));
        assert.equal(config.installation.bundlePath, join(directory, "bundle.json"));
        assert.equal(config.installation.source.path, join(directory, "source.json"));
        assert.equal(config.installation.alchemyConfig, join(directory, "alchemy.json"));
        assert.equal(config.installation.dataDir, join(directory, "data"));
        await writeFile(path, JSON.stringify({requestPath: "request.json", installation: {bundlePath: "bundle.json"}}));
        assert.equal((await loadConfig(path)).installation.source, undefined);
        await writeFile(path, JSON.stringify({installation: {}}));
        await assert.rejects(loadConfig(path), /requestPath/);
        await assert.rejects(loadConfig(), /configuration JSON/);
        await writeFile(path, JSON.stringify({tx: "historical-signature", installation: {}}));
        assert.equal((await loadConfig(path)).tx, "historical-signature");
        await writeFile(path, JSON.stringify({tx: "historical-signature", requestPath: "request.json", installation: {}}));
        await assert.rejects(loadConfig(path), /exactly one/);
    } finally { await rm(directory, {recursive: true}); }
});
