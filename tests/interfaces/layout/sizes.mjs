import { readdirSync, readFileSync } from "node:fs";
import { basename, relative, resolve } from "node:path";

const ignored = new Set([".git", "target", "node_modules", "dist", ".svm-replay"]);
// One indivisible opt-in historical scenario: keep setup and assertions readable.
const exceptions = new Map([
    ["src/engine/_shared/_3_debug/_5_store/tests/debug_store_execution.rs", 260],
]);

export function lineLimit(path) {
    if (exceptions.has(path)) return exceptions.get(path);
    return ["mod.rs", "main.rs", "lib.rs", "root.rs"].includes(basename(path)) ? 70 : 250;
}

export function sizeViolation(path, source) {
    const lines = source.trimEnd().split("\n").length;
    const limit = lineLimit(path);
    return lines > limit ? `${path}: ${lines} lines (ceiling ${limit}; aim for ~50/200)` : null;
}

export function auditSizes(root) {
    const violations = [];
    function visit(directory) {
        for (const entry of readdirSync(directory, { withFileTypes: true })) {
            if (ignored.has(entry.name)) continue;
            const file = resolve(directory, entry.name);
            if (entry.isDirectory()) visit(file);
            else if (/\.(rs|ts|mjs)$/.test(entry.name)) {
                const path = relative(root, file).replaceAll("\\", "/");
                const violation = sizeViolation(path, readFileSync(file, "utf8"));
                if (violation) violations.push(violation);
            }
        }
    }
    for (const directory of ["src", "tests", "examples", ".github"]) visit(resolve(root, directory));
    return violations;
}
