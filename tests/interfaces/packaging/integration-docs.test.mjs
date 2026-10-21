import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = path => readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8");

test("README links its rationale and presents documentation as a vertical list", () => {
    const readme = read("README.md");
    assert.doesNotMatch(readme, /^## Why this exists$/m);
    const docs = readme.split("## Documentation\n")[1].split("\nOwned code:")[0].trim();
    assert.ok(docs.startsWith("- [Why SVM Replay?](docs/why-svm-replay.md)"));
    for (const line of docs.split("\n").filter(line => line.trim())) assert.match(line, /^- /);
    assert.match(docs, /SDKs:.*TypeScript.*Rust/);
    const rationale = read("docs/why-svm-replay.md");
    assert.match(rationale, /effects of earlier transactions/);
    assert.match(rationale, /runtime\nrules/);
    assert.match(rationale, /\[provider coverage\]\(providers\.md\)/);
});

test("Anchor's primary recipe owns a Surfpool fork instead of the limited standalone server", () => {
    const anchor = read("README.md").split("## Anchor\n")[1].split("\n## ")[0];
    assert.match(anchor, /HistoricalFork\.start/);
    assert.doesNotMatch(anchor, /Replay\.installed|skip_local_validator = true/);
    assert.match(anchor, /Surfpool/);
    assert.match(anchor, /generated test.*before.*fund|before.*hook.*fund/s,
        "the abbreviated snippet must point to funding, ELF installation and new-account setup");
    assert.match(anchor, /anchor build --arch v0\nanchor test --skip-build --skip-deploy/,
        "test must not rebuild the explicit v0 program with the default architecture");
});

test("integration instructions do not recommend the obsolete datasource execution route", () => {
    const integrations = read("docs/integrations.md");
    assert.match(integrations, /anchor build --arch v0\nanchor test --skip-build --skip-deploy/);
    assert.doesNotMatch(integrations, /svm-replay fork start|--allow-new-accounts|--detach/);
    assert.match(integrations, /Standalone.*limited/i);
    assert.match(integrations, /anchor localnet.*unchanged/s);
    for (const key of ["account_dir", "test.genesis"])
        assert.ok(integrations.includes(key), `document historical refusal of ${key}`);
    assert.match(integrations, /test\.validator\.clone[^\n]*addresses[^\n]*hydrate/);
    assert.match(read("docs/tool-integration.md"), /Surfpool.*public RPC/);
    assert.match(read("src/bindings/typescript/README.md"), /not.*historical execution.*Surfpool/i);
    assert.doesNotMatch(read("docs/limitations.md"), /Historical forks execute in Surfpool 1\.6\.0|--allow-new-accounts|--new-account/);
});

test("the retained old Anchor datasource fixture is not advertised as full historical execution", () => {
    assert.match(read("examples/anchor/historical/README.md"), /Legacy datasource fixture/);
    assert.match(read("examples/README.md"), /Legacy datasource fixture/);
});

test("historical CI requires reviewed tools and runs the test-owned fork route", () => {
    const workflow = read("examples/ci/historical.yml");
    assert.doesNotMatch(workflow, /fork start|--match-datasource|--detach/);
    for (const pin of ["ANCHOR_SHA256", "SURFPOOL_SHA256", "SVM_REPLAY_SHA256"]) {
        assert.ok(workflow.includes(pin), `missing ${pin}`);
    }
    assert.match(workflow, /anchor test --skip-deploy/);
    const beforeTestStep = workflow.split("- name: Run test-owned historical forks")[0];
    assert.doesNotMatch(beforeTestStep, /API_ALCHEMY:/,
        "checkout, verification and dependency installation must not receive the provider secret");
    assert.match(workflow.split("- name: Run test-owned historical forks")[1], /env:\s*\n\s*API_ALCHEMY:/);
});
