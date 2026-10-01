import { existsSync, readdirSync, readFileSync } from "node:fs";
import { relative, resolve } from "node:path";

const ignored = new Set(["target", "node_modules", "dist", ".git"]);
const testDirectory = name => name === "tests";

export function componentName(name) {
    if (/^_\d+_tests$/.test(name)) return false;
    return /^_\d+_[a-z0-9_]+$/.test(name) || name === "_shared";
}

export function leafName(name, entry) {
    return name === entry || name === "tests.rs" || /^_\d+_[a-z0-9_]+\.rs$/.test(name);
}

export function looseRoutingFiles(names, hasComponents) {
    if (!names.includes("mod.rs") || !hasComponents) return [];
    return names.filter(name => name.endsWith(".rs") && !["mod.rs", "tests.rs"].includes(name));
}

export function auditHierarchy(root) {
    const violations = [];
    function visit(directory, entry) {
        const entries = readdirSync(directory, { withFileTypes: true });
        const children = entries.filter(entry => entry.isDirectory()
            && !ignored.has(entry.name) && !testDirectory(entry.name));
        const names = entries.filter(entry => entry.isFile()).map(entry => entry.name);
        if (entries.some(entry => entry.isDirectory() && testDirectory(entry.name))) {
            const owned = readdirSync(resolve(directory, "tests"), { withFileTypes: true });
            if (owned.length === 1 && owned[0].isFile() && owned[0].name.endsWith(".rs")) {
                violations.push(`${relative(root, directory)}/tests: single test file belongs in sibling tests.rs`);
            }
            if (names.includes("tests.rs")) violations.push(`${relative(root, directory)}: use tests.rs or tests/, not both`);
        }
        for (const file of looseRoutingFiles(names.map(name => name === entry ? "mod.rs" : name), children.length > 0)) {
            violations.push(`${relative(root, directory)}/${file}: routing directories contain only mod.rs; move implementation into an owned component`);
        }
        const numbers = new Set();
        for (const child of children) {
            const path = resolve(directory, child.name);
            if (!componentName(child.name)) violations.push(`${relative(root, path)}: expected numbered component or _shared`);
            const number = child.name.match(/^_(\d+)_/)?.[1];
            if (number && numbers.has(number)) violations.push(`${relative(root, path)}: duplicate sibling number ${number}`);
            if (number) numbers.add(number);
            if (!existsSync(resolve(path, "mod.rs"))) violations.push(`${relative(root, path)}: missing mod.rs`);
            visit(path, "mod.rs");
        }
        for (const name of names.filter(name => name.endsWith(".rs"))) {
            if (!leafName(name, entry)) violations.push(`${relative(root, directory)}/${name}: implementation files must be numbered`);
            const text = readFileSync(resolve(directory, name), "utf8");
            for (const [, target] of text.matchAll(/#\[path = "([^"]+)"\]/g)) {
                if (!target.split("/").includes("tests") && target.split("/").at(-1) !== "tests.rs") violations.push(`${relative(root, directory)}/${name}: production #[path] indirection`);
            }
        }
    }
    for (const area of ["cli", "engine", "protocol", "store"]) {
        visit(resolve(root, "src", area), area === "cli" ? "main.rs" : "mod.rs");
    }
    return violations;
}
