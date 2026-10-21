import {test} from "node:test";
import assert from "node:assert/strict";
import {readdirSync, readFileSync} from "node:fs";
import {join} from "node:path";
import {root} from "./fixtures.mjs";

test("native installer entries stay thin and use numbered platform owners",()=>{
    assert.deepEqual(readdirSync(join(root,"src/installer")).sort(),["_0_unix","_1_windows","_2_release"]);
    for(const file of ["install.sh","install.ps1"]){
        const text=readFileSync(join(root,file),"utf8");
        assert.ok(text.trimEnd().split("\n").length<=50,file);
        assert.match(text,/src\/installer\//);
        assert.doesNotMatch(text,/npm|node |cargo |rustup|ExecutionPolicy|Unblock-File/);
    }
    for(const file of ["_0_unix/main.sh","_0_unix/_0_archive.sh","_0_unix/_1_install.sh",
        "_1_windows/main.ps1","_1_windows/_0_archive.ps1","_1_windows/_1_install.ps1"]){
        const text=readFileSync(join(root,"src/installer",file),"utf8");
        assert.ok(text.trimEnd().split("\n").length<=200,file);
        assert.doesNotMatch(text,/npm|node |cargo |rustup|ExecutionPolicy|Unblock-File/);
    }
});

test("the default quick start is a native install followed by a real demo",()=>{
    const readme=readFileSync(join(root,"README.md"),"utf8");
    const section=readme.split("## Quick start\n")[1].split("\n## ")[0];
    assert.match(section,/curl -fsSL https:\/\/raw\.githubusercontent\.com\/xternet\/svm-replay\/v0\.2\.0\/install\.sh \| sh/);
    assert.match(section,/svm-replay demo/);
    assert.match(section,/no (?:Node\.js|Node)/i);
    const commands=section.match(/```sh\n([\s\S]*?)```/)[1];
    assert.doesNotMatch(commands,/npm install|cargo (?:install|add)/);
    const windows=section.match(/```powershell\n([\s\S]*?)```/)?.[1];
    assert.ok(windows,"Windows PowerShell needs its own quick-start commands");
    assert.match(windows,/https:\/\/raw\.githubusercontent\.com\/xternet\/svm-replay\/v0\.2\.0\/install\.ps1/);
    assert.match(windows,/\$env:LOCALAPPDATA\\svm-replay\\bin/);
    assert.match(windows,/svm-replay demo/);
    assert.doesNotMatch(windows,/ExecutionPolicy|Unblock-File|npm install|cargo (?:install|add)/);
    const sdkNote=section.split("\n").find(line=>line.startsWith("<sub>SDKs"));
    assert.ok(sdkNote,"SDK installation belongs in one secondary line");
    assert.match(sdkNote,/npm install @xternet\/svm-replay/);
    assert.match(sdkNote,/cargo add svm-replay-engine/);
    assert.match(sdkNote,/native bundle required/);
});
