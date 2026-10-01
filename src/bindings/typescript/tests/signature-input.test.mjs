import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,readFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {materializeSignatureInputs} from "../dist/signature-input.js";
import {signatureArgs} from "../dist/signature.js";
test("collection file and all survive SDK materialization",async()=>{
    const dir=await mkdtemp(join(tmpdir(),"svm-collect-test-"));
    try {
        const tx="1".repeat(64);
        const options=await materializeSignatureInputs({tx,collect:{calls:true,maxEvents:100}},dir);
        assert.deepEqual(JSON.parse(await readFile(options.collect.path)),{calls:true,maxEvents:100});
        const all=await materializeSignatureInputs({tx,collect:"all"},dir);
        assert.equal(all.collect,"all");
    } finally {await rm(dir,{recursive:true});}
});
test("large SDK inputs become exact private files, not process arguments",async()=>{
    const dir=await mkdtemp(join(tmpdir(),"svm-input-test-"));
    try {
        const original={tx:"1".repeat(64),overrides:[{pubkey:"11111111111111111111111111111111",dataBase64:Buffer.alloc(200000).toString("base64")}],alchemyConfig:"provider.json"};
        const options=await materializeSignatureInputs(original,dir);
        assert.deepEqual(JSON.parse(await readFile(options.overrides.path)),original.overrides);
        assert.ok(signatureArgs(options,dir).every(arg=>arg.length<4096));
        assert.ok(signatureArgs(options,dir).includes("--alchemy-config"));
        assert.equal(Array.isArray(original.overrides),true);
    } finally {await rm(dir,{recursive:true});}
});
test("ambiguous SDK provider budgets reject before launching the CLI",()=>{
    for(const limit of [{maxRequests:2},{maxDownloadBytes:1024}]){
        assert.throws(()=>signatureArgs({tx:"1".repeat(64),alchemyConfig:"provider.json",...limit},"out"),
            error=>error.code==="INVALID_REQUEST");
    }
});
