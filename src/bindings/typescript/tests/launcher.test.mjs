import {test} from "node:test";
import assert from "node:assert/strict";
import {activationPath} from "../installation.mjs";
import {activate,installedBinary} from "../installation.mjs";
import {mkdtemp,mkdir,writeFile,readFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {createHash} from "node:crypto";
test("activation uses OS application data and never edits shell profiles",()=>{
    assert.equal(activationPath({XDG_DATA_HOME:"/data"},"linux"),join("/data","svm-replay","command.json"));
    assert.throws(()=>activationPath({},"win32"),/LOCALAPPDATA/);
    assert.throws(()=>activationPath({},"unknown"),/unsupported/);
});
test("activation verifies local pins and rejects changed CLI bytes",async()=>{
    const root=await mkdtemp(join(tmpdir(),"svm-activation-"));
    const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
    try {
        await mkdir(join(root,"bin"));
        const name=process.platform==="win32"?"bin/svm-replay.exe":"bin/svm-replay";
        const bytes=Buffer.from("identity test only; never executed");
        await writeFile(join(root,name),bytes);
        const manifest=JSON.stringify({schema:"svm-replay-bundle/v1",binary:name,files:[{path:name,executable:true,sha256:hash(bytes)}]});
        await writeFile(join(root,"bundle.json"),manifest);
        await writeFile(join(root,"installation.json"),JSON.stringify({schema:"svm-replay-installation-pin/v1",bundleSha256:hash(manifest)}));
        const config=join(root,"appdata","command.json");
        await activate(root,config);
        assert.equal(JSON.parse(await readFile(config)).directory,root);
        assert.equal(await installedBinary(root),join(root,name));
        await writeFile(join(root,name),"tampered");
        await assert.rejects(installedBinary(root),/hash differs/);
    } finally {await rm(root,{recursive:true});}
});
