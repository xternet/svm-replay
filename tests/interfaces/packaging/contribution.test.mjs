import { test } from "node:test";
import assert from "node:assert/strict";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const read = file => readFileSync(resolve(root, file), "utf8");

test("Git ignores nested credentials but includes the documented environment template", () => {
    const directory = mkdtempSync(join(tmpdir(), "svm-replay-ignore-"));
    try {
        execFileSync("git", ["init", "--quiet", directory]);
        copyFileSync(resolve(root, ".gitignore"), join(directory, ".gitignore"));
        const secrets = [".env", ".env.production", "nested/.env", "nested/deep/.env.local"];
        const template = "examples/config/.env.example";
        const ignored = execFileSync("git", ["-c", "core.excludesFile=/dev/null", "check-ignore", "--no-index", "--stdin"],
            { cwd: directory, encoding: "utf8", input: [...secrets, template].join("\n") + "\n" });
        assert.deepEqual(ignored.trim().split("\n"), secrets);
    } finally {
        rmSync(directory, {recursive: true, force: false});
    }
});

test("local Rust pin matches CI and CI runs package/contribution checks after Rust setup", () => {
    const pin = read("rust-toolchain.toml").match(/^channel = "([^"]+)"$/m)?.[1];
    assert.ok(pin, "missing Rust toolchain pin");
    const workflow = read(".github/workflows/ci.yml");
    const setup = workflow.indexOf(`rustup toolchain install ${pin}`);
    assert.ok(setup >= 0, "CI must install the pinned toolchain");
    assert.ok(workflow.includes(`cargo +${pin} fmt --all --check`));
    assert.ok(workflow.includes(`cargo +${pin} test --workspace --locked`));
    for (const file of ["metadata", "contribution"]) {
        assert.ok(workflow.indexOf(`tests/interfaces/packaging/${file}.test.mjs`) > setup,
            `${file} checks must run after Rust setup`);
    }
});

test("contributor templates point to checks and private security reporting", () => {
    assert.match(read("SECURITY.md"), /mailto:pm@xternet\.dev/);
    const pr = read(".github/pull_request_template.md");
    assert.match(pr, /regression test/i);
    assert.match(pr, /secret/i);
    const issue = read(".github/ISSUE_TEMPLATE/bug_report.md");
    assert.match(issue, /^name: Bug report$/m);
    assert.match(issue, /SECURITY\.md/);
    assert.match(issue, /runtime/i);
    assert.match(issue, /redact/i);
});

test("SDK PR checks cover Linux, macOS and Windows without private inputs", () => {
    const workflow = read(".github/workflows/ci.yml");
    assert.match(workflow, /portable-sdk:/);
    assert.match(workflow, /os: \[ubuntu-24\.04, macos-14, windows-2022\]/);
    assert.match(workflow, /runs-on: \$\{\{ matrix\.os \}\}/);
    assert.match(workflow, /fail-fast: false/);
    const portable = workflow.split("  portable-sdk:")[1].split("  offline-checks:")[0];
    for (const command of ["npm ci --ignore-scripts", "npm test", "npm pack --dry-run"]) {
        assert.ok(portable.includes(`run: ${command}\n`),
            `${command} needs a separate step so PowerShell cannot hide its exit code`);
    }
});
