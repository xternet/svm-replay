import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";

test("optional npm recipe installs native workers before invoking its launcher", () => {
    const guide = readFileSync(new URL("../../../docs/installation.md", import.meta.url), "utf8");
    const section = guide.split("To get the short command through npm (after publication):\n")[1]?.split("### Cached offline demo")[0];
    assert.ok(section, "npm recipe is required alongside the native quick start");
    const commands = section.match(/```sh\n([\s\S]*?)```/)?.[1];
    assert.ok(commands, "npm shell commands are required");
    assert.match(commands, /npm install -g @xternet\/svm-replay@0\.2\.0/);
    const installer = commands.indexOf("svm-replay-install ");
    const demo = commands.indexOf("svm-replay --demo");
    assert.ok(installer >= 0 && demo > installer, "Native installation must precede demo");
    assert.match(commands, /--release \.\/release\.json/);
    assert.match(commands, /--sha256 "\$RELEASE_SHA256"/);
    assert.match(commands, /--activate/);
    assert.match(guide, /download/i, "Explain where the native archive comes from");
    assert.match(guide, /trusted/i, "Do not derive trust merely by hashing the download");
    assert.match(guide, /not.*native|native.*not/i, "State npm's installation scope");
});
