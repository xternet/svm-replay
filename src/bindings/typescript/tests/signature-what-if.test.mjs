import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,readFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {historicalRequest} from "../dist/index.js";
import {materializeSignatureInputs} from "../dist/signature-input.js";
import {signatureArgs} from "../dist/signature.js";
import {verifySignatureOutput} from "../dist/signature-output.js";

const tx="1".repeat(64);
const bundle=[{transactionBase64:"AQ=="},{transactionBase64:"Ag=="}];
const overrides=[{pubkey:"11111111111111111111111111111111",program:"@arb.so"}];

test("bundles go to the CLI after or instead of the target, as private files",async()=>{
    const dir=await mkdtemp(join(tmpdir(),"svm-what-if-test-"));
    try {
        const after=await materializeSignatureInputs({tx,append:bundle,overrides},dir);
        assert.deepEqual(JSON.parse(await readFile(after.append.path)),bundle);
        assert.deepEqual(JSON.parse(await readFile(after.overrides.path)),overrides);
        const args=signatureArgs(after,dir);
        assert.equal(args[args.indexOf("--append")+1],"@"+after.append.path);
        assert.ok(!args.includes("--replace"));
        const instead=signatureArgs(await materializeSignatureInputs({tx,replace:bundle},dir),dir);
        assert.ok(instead.includes("--replace")&&!instead.includes("--append"));
    } finally {await rm(dir,{recursive:true});}
    assert.throws(()=>signatureArgs({tx,append:bundle,replace:bundle},"output"),e=>e.code==="INVALID_REQUEST");
});

test("request builder carries a what-if bundle only when given",()=>{
    const input={requestId:"test",family:"v4-2",genesisHash:"explicit",candidate:{slot:1,transactionIndex:0},runtimeBinding:{targetSlot:1}};
    assert.equal("whatIf" in historicalRequest(input),false);
    const whatIf={mode:"after",transactionsBase64:["AQ=="],stateOverrides:[{pubkey:"x",lamports:"0"}]};
    assert.deepEqual(historicalRequest({...input,whatIf}).whatIf,whatIf);
});

test("selected fields apply to every bundle result",async()=>{
    const dir=await mkdtemp(join(tmpdir(),"svm-what-if-out-"));
    try {
        const {writeFile}=await import("node:fs/promises");const {createHash}=await import("node:crypto");
        const run={status:"ok",logs:["a"],computeUnits:5,accountTransitions:[]};
        const result={original:{...run},whatIf:{mode:"after",stateOverrides:[],transactions:[{...run},{...run}]}};
        const bytes=Buffer.from(JSON.stringify(result));
        await writeFile(join(dir,"result.json"),bytes);
        const output={file:"result.json",sha256:createHash("sha256").update(bytes).digest("hex")};
        const filtered={original:{status:"ok"},whatIf:{mode:"after",stateOverrides:[],transactions:[{status:"ok"},{status:"ok"}]}};
        await verifySignatureOutput({result:filtered},{outcome:"COMPLETED",output},dir,["status"],1<<20);
        await assert.rejects(verifySignatureOutput({result},{outcome:"COMPLETED",output},dir,["status"],1<<20));
    } finally {await rm(dir,{recursive:true});}
});
