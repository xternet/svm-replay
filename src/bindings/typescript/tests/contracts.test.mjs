import {test} from "node:test";
import assert from "node:assert/strict";
import {historicalRequest, encodeJson, ReplayError, readArtifact} from "../dist/index.js";
import {mkdtemp,writeFile,symlink,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {createHash} from "node:crypto";

test("builder preserves explicit historical evidence and wide integers", () => {
    const input={requestId:"test",family:"v4-2",genesisHash:"explicit",candidate:{slot:123,transactionIndex:4},runtimeBinding:{targetSlot:123},
        requestedAccountOverrides:[{address:"explicit",lamports:"18446744073709551615"}]};
    const request=historicalRequest(input);
    assert.equal(request.schema,"svm-replay-historical/v1");
    assert.equal(request.metadataPolicy,"STRICT");
    assert.equal(request.requestedAccountOverrides[0].lamports,"18446744073709551615");
    request.candidate.slot=124;
    assert.equal(input.candidate.slot,123,"builder must not alias caller objects");
    assert.throws(()=>encodeJson({balance:9007199254740992}),e=>e.code==="UNSAFE_NUMBER");
    assert.throws(()=>encodeJson({amount:1n}),e=>e.code==="INVALID_JSON");
    assert.throws(()=>historicalRequest({...input,family:"latest"}),e=>e.code==="UNSUPPORTED_RUNTIME");
    assert.throws(()=>historicalRequest({...input,metadataPolicy:"ignore"}),e=>e.code==="INVALID_REQUEST");
});

test("builder admits reviewed early runtime without admitting unknown families", () => {
    const input={requestId:"early",family:"v2-2",genesisHash:"explicit",
        candidate:{slot:353951234,transactionIndex:0},runtimeBinding:{targetSlot:353951234}};
    assert.equal(historicalRequest(input).family,"v2-2");
    assert.throws(()=>historicalRequest({...input,family:"v2-1"}),e=>e.code==="UNSUPPORTED_RUNTIME");
});

test("artifacts are bounded, hash checked and confined", async () => {
    const root=await mkdtemp(join(tmpdir(),"replay-sdk-"));
    try {
        const bytes=Buffer.from("immutable evidence"), sha256=createHash("sha256").update(bytes).digest("hex");
        await writeFile(join(root,"result.json"),bytes);
        const artifact={file:"result.json",sha256};
        assert.deepEqual(await readArtifact(root,artifact,1024),bytes);
        await assert.rejects(readArtifact(root,artifact,1),e=>e.code==="OUTPUT_LIMIT");
        await assert.rejects(readArtifact(root,{...artifact,file:"../result.json"},1024),e=>e.code==="ARTIFACT_PATH");
        await symlink(join(root,"result.json"),join(root,"link"));
        await assert.rejects(readArtifact(root,{...artifact,file:"link"},1024),ReplayError);
        await writeFile(join(root,"result.json"),"changed");
        await assert.rejects(readArtifact(root,artifact,1024),e=>e.code==="ARTIFACT_INTEGRITY");
    } finally { await rm(root,{recursive:true,force:true}); }
});
