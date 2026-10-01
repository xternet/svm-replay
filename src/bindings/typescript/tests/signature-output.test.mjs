import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {digest} from "../dist/io.js";
import {verifySignatureOutput} from "../dist/signature-output.js";
test("incomplete evidence is not promoted to a successful SDK result",async()=>{
    const receipt={outcome:"UNSUPPORTED",incomplete:{category:"HISTORICAL_ACCOUNT_MISSING",estimated:false,available:{}}};
    assert.equal(await verifySignatureOutput(receipt,receipt,"/unused",[],4096),undefined);
    await assert.rejects(verifySignatureOutput({...receipt,result:{}},receipt,"/unused",[],4096),e=>e.code==="RECEIPT_INTEGRITY");
});
test("SDK checks returned values against real hashed result and trace files",async()=>{
    const root=await mkdtemp(join(tmpdir(),"svm-artifact-test-"));
    try {
        const result={original:{status:"ok",computeUnits:150,logs:[]}};
        const trace=[{kind:"journal_scope"}];
        const save=async(file,value)=>{const bytes=Buffer.from(JSON.stringify(value));await writeFile(join(root,file),bytes);return {file,sha256:digest(bytes)};};
        const output=await save("result.json",result),exp=await save("trace-original-control.json",trace);
        const receipt={outcome:"COMPLETED",output,trace:{exports:[exp]}};
        const full={result,trace:{exports:[{...exp,data:trace}]}};
        await verifySignatureOutput(full,receipt,root,[],4096);
        await assert.rejects(verifySignatureOutput({...full,result:{original:{status:"invented"}}},receipt,root,[],4096),e=>e.code==="RECEIPT_INTEGRITY");
        await assert.rejects(verifySignatureOutput({...full,trace:{exports:[{...exp,data:[]}]}},receipt,root,[],4096),e=>e.code==="RECEIPT_INTEGRITY");
        await verifySignatureOutput({result:{original:{status:"ok"}},trace:{exports:[exp]}},receipt,root,["status"],4096);
        await writeFile(join(root,exp.file),"[]");
        await assert.rejects(verifySignatureOutput(full,receipt,root,[],4096),e=>e.code==="ARTIFACT_INTEGRITY");
    } finally {await rm(root,{recursive:true});}
});
