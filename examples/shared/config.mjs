import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { Replay } from "@xternet/svm-replay";

export async function loadConfig(path) {
    if (!path) throw new Error("pass a local configuration JSON path (see examples/README.md)");
    const file = resolve(path);
    const config = JSON.parse(await readFile(file, "utf8"));
    const hasTx = typeof config.tx === "string" && config.tx.length > 0;
    const hasRequest = typeof config.requestPath === "string" && config.requestPath.length > 0;
    if (hasTx === hasRequest) throw new Error("provide exactly one of tx or requestPath");
    if (!config.installation || typeof config.installation !== "object") throw new Error("installation is required");
    const absolute = value => resolve(dirname(file), value);
    if (hasRequest) config.requestPath = absolute(config.requestPath);
    for (const key of ["bundlePath", "dataDir", "alchemyConfig"]) {
        if (config.installation[key] !== undefined) config.installation[key] = absolute(config.installation[key]);
    }
    if (config.installation.source !== undefined) {
        config.installation.source.path = absolute(config.installation.source.path);
    }
    return config;
}

export async function configured(path) {
    const config = await loadConfig(path);
    const replay = await Replay.open(config.installation);
    return { config, replay, options: config.tx === undefined ? { request: { path: config.requestPath } } : { tx: config.tx } };
}
