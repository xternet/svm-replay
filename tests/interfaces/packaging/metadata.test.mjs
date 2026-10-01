import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { execFileSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const read = file => readFileSync(resolve(root, file), "utf8");

test("release packages use the approved owner and registry metadata", () => {
    for (const file of ["Cargo.toml", "src/engine/Cargo.toml", "src/protocol/Cargo.toml", "src/store/Cargo.toml"]) {
        const manifest = read(file);
        assert.match(manifest, /^repository = "https:\/\/github.com\/xternet\/svm-replay"$/m);
        assert.match(manifest, /^publish = \["crates-io"\]$/m);
    }
    const sdk = JSON.parse(read("src/bindings/typescript/package.json"));
    assert.equal(sdk.name, "@xternet/svm-replay");
    assert.equal(sdk.publishConfig.access, "public");
    assert.equal(sdk.repository.url, "git+https://github.com/xternet/svm-replay.git");
    assert.equal(JSON.parse(read("examples/package.json")).dependencies[sdk.name], "file:../src/bindings/typescript");
});

test("Rust packages specify versions for internal path dependencies", () => {
    for (const file of ["Cargo.toml", "src/engine/Cargo.toml", "src/protocol/Cargo.toml", "src/store/Cargo.toml"]) {
        const manifest = read(file);
        const version = manifest.match(/^version = "([^"]+)"/m)?.[1];
        assert.ok(version, `${file}: missing package version`);
        for (const line of manifest.split("\n").filter(line => /^svm-replay.*path =/.test(line))) {
            assert.ok(line.includes(`version = "=${version}"`), `${file}: registry version missing: ${line}`);
        }
        assert.match(manifest, /^description = ".+"$/m, `${file}: missing package description`);
    }
});

test("packaged engine test helpers stay inside the engine crate", () => {
    const engine = resolve(root, "src/engine");
    function walk(directory) {
        for (const entry of readdirSync(directory, {withFileTypes:true})) {
            const path = resolve(directory, entry.name);
            if (entry.isDirectory()) walk(path);
            else if (path.endsWith(".rs")) {
                for (const [, target] of readFileSync(path, "utf8").matchAll(/#\[path = "([^"]+)"\]/g)) {
                    assert.ok(resolve(dirname(path), target).startsWith(engine + "/"), `unpackaged test dependency: ${path}: ${target}`);
                }
            }
        }
    }
    walk(engine);
});

test("CLI and SDK release versions agree", () => {
    const version = read("Cargo.toml").match(/^version = "([^"]+)"/m)?.[1];
    assert.equal(JSON.parse(read("src/bindings/typescript/package.json")).version, version);
});

test("CLI crate excludes other packages and development dependencies", () => {
    const files = execFileSync("cargo", ["package", "--list", "--allow-dirty", "--locked", "--offline", "-p", "svm-replay"],
        { cwd: root, encoding: "utf8", maxBuffer: 4 * 1024 * 1024 }).trim().split("\n");
    const rootFiles = new Set(["Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "README.md", "CHANGELOG.md", "LICENSE", "NOTICE", ".cargo_vcs_info.json"]);
    const unexpected = files.filter(file => !rootFiles.has(file) && !file.startsWith("src/cli/") && !file.startsWith("examples/rust/"));
    assert.deepEqual(unexpected.slice(0, 10), [], "CLI package must contain only its release inputs");
    assert.ok(files.includes("src/cli/main.rs") && files.includes("LICENSE") && files.includes("NOTICE"));
});

test("owned Rust and npm packages carry Apache-2.0 and the full license", () => {
    const license = read("LICENSE");
    assert.match(license, /Apache License\s+Version 2\.0, January 2004/);
    assert.match(license, /END OF TERMS AND CONDITIONS/);
    for (const directory of [".", "src/engine", "src/protocol", "src/store"]) {
        assert.match(read(`${directory}/Cargo.toml`), /^license = "Apache-2.0"$/m);
        assert.equal(read(`${directory}/LICENSE`), license);
        assert.equal(read(`${directory}/NOTICE`), read("NOTICE"));
    }
    const sdk = JSON.parse(read("src/bindings/typescript/package.json"));
    assert.equal(sdk.license, "Apache-2.0");
    assert.ok(sdk.files.includes("LICENSE") && sdk.files.includes("NOTICE"));
    assert.equal(read("src/bindings/typescript/LICENSE"), license);
    assert.equal(read("src/bindings/typescript/NOTICE"), read("NOTICE"));
});
