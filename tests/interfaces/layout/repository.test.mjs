import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { auditSizes, sizeViolation } from "./sizes.mjs";
import { auditHierarchy, componentName, leafName, looseRoutingFiles } from "./hierarchy.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const ignored = new Set([".git", "target", "node_modules", "dist", ".svm-replay"]);
test("recursive guard rejects unnumbered, missing-entry and duplicate-step modules", () => {
    const fixture = mkdtempSync(resolve(tmpdir(), "svm-layout-"));
    try {
        for (const name of ["cli", "engine", "protocol", "store"]) {
            mkdirSync(resolve(fixture, "src", name), { recursive: true });
            writeFileSync(resolve(fixture, "src", name, name === "cli" ? "main.rs" : "mod.rs"), "");
        }
        assert.deepEqual(auditHierarchy(fixture), []);
        mkdirSync(resolve(fixture, "src/engine/tests"));
        writeFileSync(resolve(fixture, "src/engine/tests/case.rs"), "// Owned tests are unnumbered.\n");
        assert.match(auditHierarchy(fixture).join("\n"), /single test file/);
        writeFileSync(resolve(fixture, "src/engine/tests/second.rs"), "// Multiple files keep their directory.\n");
        writeFileSync(resolve(fixture, "src/cli/tests.rs"), "// One file stays beside its owner.\n");
        assert.deepEqual(auditHierarchy(fixture), []);
        writeFileSync(resolve(fixture, "src/engine/tests.rs"), "// Cannot coexist with tests/.\n");
        assert.match(auditHierarchy(fixture).join("\n"), /not both/);
        rmSync(resolve(fixture, "src/engine/tests.rs"));
        for (const name of ["catalog", "_0_first", "_0_duplicate"]) mkdirSync(resolve(fixture, "src/engine", name));
        writeFileSync(resolve(fixture, "src/engine/mod.rs"), '#[path = "catalog/load.rs"]\nmod catalog;\n');
        const errors = auditHierarchy(fixture).join("\n");
        assert.match(errors, /expected numbered component/);
        assert.match(errors, /missing mod.rs/);
        assert.match(errors, /duplicate sibling number/);
        assert.match(errors, /production #\[path\]/);
    } finally { rmSync(fixture, { recursive: true, force: true }); }
});
test("routing modules have one index and implementation in child components", () => {
    assert.equal(leafName("implementation.rs", "mod.rs"), false);
    assert.equal(leafName("_0_implementation.rs", "mod.rs"), true);
    assert.equal(leafName("mod.rs", "mod.rs"), true);
    assert.equal(leafName("tests.rs", "mod.rs"), true);
    assert.equal(componentName("catalog"), false);
    assert.equal(componentName("_9_tests"), false);
    assert.equal(componentName("_0_catalog"), true);
    assert.equal(componentName("_shared"), true);
    assert.deepEqual(looseRoutingFiles(["mod.rs", "_1_resolve_runtime.rs", "resolve.rs"], true),
        ["_1_resolve_runtime.rs", "resolve.rs"]);
    assert.deepEqual(looseRoutingFiles(["mod.rs", "profiles.rs"], false), []);
    assert.deepEqual(looseRoutingFiles(["mod.rs"], true), []);
    assert.deepEqual(looseRoutingFiles(["mod.rs", "tests.rs"], true), []);
    assert.deepEqual(auditHierarchy(root), []);
});
test("size guard allows small overruns but rejects oversized modules", () => {
    const lines = count => Array(count).fill("code").join("\n");
    assert.equal(sizeViolation("src/example/mod.rs", lines(55)), null);
    assert.equal(sizeViolation("src/example/helper.rs", lines(201)), null);
    assert.match(sizeViolation("src/example/mod.rs", lines(71)), /ceiling 70/);
    assert.match(sizeViolation("src/example/helper.rs", lines(251)), /ceiling 250/);
    assert.deepEqual(auditSizes(root), []);
});
function entries(directory, allowed) {
    validateNames(readdirSync(resolve(root, directory)), allowed, directory);
}
function validateNames(names, allowed, directory) {
    for (const entry of names) {
        // CLI review exports are local data, not source components.
        if (directory === "." && ["result", "results"].includes(entry)) continue;
        if (ignored.has(entry) || /^\..+\.sw[p-z]$/.test(entry)) continue;
        assert.ok(allowed.includes(entry), `${directory}/${entry}: assign an owner and update the documented layout`);
    }
}

test("layout rules reject loose files and unowned folders", () => {
    validateNames(["result", "results"], [], ".");
    assert.throws(() => validateNames(["results"], [], "src"), /assign an owner/);
    for (const name of ["result", "results"]) {
        assert.ok(readFileSync(resolve(root, ".gitignore"), "utf8").split("\n").includes(`/${name}/`));
    }
    assert.throws(() => validateNames(["loose.rs"], ["src"], "."), /assign an owner/);
    assert.throws(() => validateNames(["misc"], ["cli", "sdk"], "tests/interfaces"), /assign an owner/);
});

test("native implementations stay in named platform modules", () => {
    for (const [owner, platforms] of Object.entries({
        "src/engine/_shared/_11_runtime/_0_process": ["_1_unix", "_0_windows"],
        "src/cli/_2_debugger": ["_1_unix", "_0_windows"],
        "src/cli/_4_lifecycle/_1_worker_owner": ["_1_linux", "_0_macos"],
        "src/cli/_4_lifecycle/_0_cancellation": ["_0_unix", "_1_windows"],
    })) {
        for (const platform of platforms) {
            assert.ok(existsSync(resolve(root, owner, platform, "mod.rs")), `${owner}/${platform}/mod.rs`);
        }
        const shared = readFileSync(resolve(root, owner, "mod.rs"), "utf8");
        assert.ok(!/libc::|windows_sys::/.test(shared), `${owner}: native calls belong in platform modules`);
    }
});
function files(directory) {
    return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
        if (ignored.has(entry.name)) return [];
        const path = resolve(directory, entry.name);
        return entry.isDirectory() ? files(path) : [path];
    });
}

test("repository roots stay grouped by responsibility", () => {
    assert.ok(!existsSync(resolve(root, "src/cli/mod.rs")), "CLI has one entry: main.rs");
    entries(".", [".gitignore", ".github", "AGENTS.md", "CONTRIBUTING.md", "README.md",
        "LICENSE", "NOTICE", "CHANGELOG.md", "SECURITY.md", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
        "src", "examples", "tests", "docs"]);
    entries("src", ["cli", "engine", "protocol", "store", "bindings"]);
    entries("src/cli", ["main.rs", "tests.rs", "tests", "_0_arguments", "_1_bundle", "_2_debugger", "_3_demo", "_4_lifecycle", "_5_report", "_6_request", "_7_sources", "_8_terminal", "_9_json", "_10_run"]);
    entries("src/bindings", ["typescript"]);
    entries("tests", ["README.md", "package.json", "package-lock.json", "harness", "historical", "overrides", "cache", "tracing", "debugging", "interfaces"]);
    entries("tests/interfaces", ["README.md", "cli", "sdk", "integrations", "packaging", "layout"]);
    entries("examples", ["README.md", "package.json", "package-lock.json", "shared", "config", "historical", "surfpool", "anchor", "rust", "ci"]);
    entries("src/engine", ["Cargo.toml", "LICENSE", "NOTICE", "mod.rs", "_6_workflow", "_shared", "tests.rs", "tests", "_0_validate", "_1_resolve_runtime", "_2_prepare_state", "_3_verify_original", "_4_simulate", "_5_finalize"]);
    entries("docs", ["architecture.md", "capabilities.md", "installation.md", "integrations.md", "limitations.md", "usage.md", "providers.md", "tracing.md", "validation.md"]);
});

test("every example has an entry point and local instructions", () => {
    for (const [directory, entry] of Object.entries({historical: "main.mjs", surfpool: "main.mjs", anchor: "main.mjs", rust: "main.rs", ci: "workflow.yml"})) {
        for (const file of [entry, "README.md"]) assert.ok(existsSync(resolve(root, "examples", directory, file)), `${directory}/${file}`);
    }
    const manifest = JSON.parse(readFileSync(resolve(root, "examples/package.json"), "utf8"));
    for (const example of ["historical", "surfpool", "anchor"]) {
        assert.equal(manifest.scripts[example], `node ${example}/main.mjs`);
    }
});

test("Cargo discovers every interface Rust test and the Rust example", () => {
    const manifest = readFileSync(resolve(root, "Cargo.toml"), "utf8");
    const paths = [...manifest.matchAll(/^path = "([^"]+)"/gm)].map(match => match[1]);
    for (const path of paths) assert.ok(existsSync(resolve(root, path)), `missing Cargo target ${path}`);
    const discovered = new Set(paths.map(path => resolve(root, path)));
    for (const owner of discovered) {
        for (const [, child] of readFileSync(owner, "utf8").matchAll(/#\[path = "([^"]+)"\]/g)) {
            discovered.add(resolve(dirname(owner), child));
        }
    }
    for (const path of files(resolve(root, "tests/interfaces")).filter(path => path.endsWith(".rs"))) {
        assert.ok(discovered.has(path), `unregistered Rust test ${path}`);
    }
    assert.ok(paths.includes("examples/rust/main.rs"));
});

test("example and interface imports resolve after directory moves", () => {
    for (const area of ["examples", "tests/interfaces"]) {
        for (const path of files(resolve(root, area)).filter(path => /\.(mjs|ts|rs)$/.test(path))) {
            const source = readFileSync(path, "utf8");
            const imports = [...source.matchAll(/(?:from\s*|import\s*|new URL\(\s*)["'](\.[^"']+)["']/g)];
            const rust = path.endsWith(".rs") ? [...source.matchAll(/#\[path = "([^"]+)"\]/g)] : [];
            for (const match of [...imports, ...rust]) {
                assert.ok(existsSync(resolve(dirname(path), match[1])), `${path}: broken relative import ${match[1]}`);
            }
        }
    }
});

function headingIds(markdown) {
    const seen = new Map();
    const ids = new Set();
    for (const line of markdown.replace(/```[\s\S]*?```/g, "").split("\n")) {
        const heading = line.match(/^#{1,6}\s+(.+?)\s*#*$/);
        if (!heading) continue;
        const slug = heading[1].toLowerCase().replace(/[^\p{L}\p{N}_\-\s]/gu, "").replace(/\s/g, "-");
        const count = seen.get(slug) ?? 0;
        ids.add(count ? `${slug}-${count}` : slug);
        seen.set(slug, count + 1);
    }
    return ids;
}
test("heading checks reject missing anchors and retain duplicate headings", () => {
    const ids = headingIds("## Quick start\n## Quick start\n```sh\n# Not a heading\n```");
    assert.deepEqual([...ids], ["quick-start", "quick-start-1"]);
    assert.equal(ids.has("missing"), false);
});

test("public documentation links resolve", () => {
    const paths = ["README.md", "CONTRIBUTING.md", "SECURITY.md", ...[".github", "docs", "examples", "tests", "src"].flatMap(area => files(resolve(root, area)))]
        .map(path => resolve(root, path)).filter(path => path.endsWith(".md"));
    for (const path of paths) {
        for (const [, target] of readFileSync(path, "utf8").matchAll(/\]\(([^\s)]+)\)/g)) {
            if (/^[a-z]+:/.test(target)) continue;
            const [file, anchor] = target.split("#");
            const destination = file ? resolve(dirname(path), file) : path;
            assert.ok(existsSync(destination), `${path}: broken link ${target}`);
            if (anchor && destination.endsWith(".md")) {
                assert.ok(headingIds(readFileSync(destination, "utf8")).has(decodeURIComponent(anchor)), `${path}: missing heading ${target}`);
            }
        }
    }
});

test("public source and guides contain no operator-machine paths", () => {
    const paths = ["AGENTS.md", "README.md", "CONTRIBUTING.md", ...["src", "tests", "examples", "docs"].flatMap(area => files(resolve(root, area)))];
    for (const path of paths.filter(path => /\.(rs|ts|mjs|md|json|toml|yml)$/.test(path))) {
        const source = readFileSync(resolve(root, path), "utf8");
        assert.ok(!/\/(?:agents|home|srv)\//.test(source), `${path}: operator-machine path`);
    }
});

test("CI enforces the same layout check and keeps its example synchronized", () => {
    const workflow = readFileSync(resolve(root, ".github/workflows/ci.yml"), "utf8");
    assert.ok(workflow.includes("node --test tests/interfaces/layout/repository.test.mjs"));
    assert.equal(workflow, readFileSync(resolve(root, "examples/ci/workflow.yml"), "utf8"));
});
