// Release preparation only; consumers need no JavaScript runtime.
import {createHash} from "node:crypto";
import {createReadStream} from "node:fs";
import {readFile, writeFile} from "node:fs/promises";
import {dirname, resolve} from "node:path";

const [manifestPath, output, ...extra] = process.argv.slice(2);
if (!manifestPath || !output || extra.length) throw new Error("Usage: checksums.mjs release.json NEW_SHA256SUMS");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
if (manifest.version !== "0.2.0" || !Array.isArray(manifest.platforms) || !manifest.platforms.length) {
    throw new Error("Expected a nonempty 0.2.0 release manifest");
}
const names = new Set(), platforms = new Set(), lines = [];
for (const row of manifest.platforms) {
    if (typeof row.file !== "string" || !/^[a-zA-Z0-9_.+-]+\.tar\.gz$/.test(row.file) ||
        typeof row.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(row.sha256)) throw new Error("Invalid archive entry");
    if (names.has(row.file) || platforms.has(row.platform)) throw new Error("duplicate archive/platform");
    names.add(row.file); platforms.add(row.platform);
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(resolve(dirname(manifestPath), row.file))) hash.update(chunk);
    const actual = hash.digest("hex");
    if (actual !== row.sha256) throw new Error(`Archive integrity mismatch: ${row.file}`);
    lines.push(`${actual}  ${row.file}\n`);
}
await writeFile(output, lines.join(""), {flag:"wx", mode:0o644});
