import {test} from "node:test";
import assert from "node:assert/strict";
import {withHistoricalReplay,anchorRequest,assertCompleted} from "../dist/adapters/index.js";

test("Surfpool companion preserves owned methods, never replaces historical authority",async()=>{
    const local={state:"current",read(){return this.state;}};const calls=[];
    const client=withHistoricalReplay(local,{simulate:async options=>{calls.push(options);return {receipt:{outcome:"COMPLETED"}};}});
    const options={request:{path:"explicit-history.json"}};
    assert.equal(client.read(),"current");await client.simulateTransactionAt(options);
    assert.deepEqual(calls,[options]);assert.equal(local.simulateTransactionAt,undefined);
    assert.throws(()=>withHistoricalReplay({simulateTransactionAt(){}},{}),e=>e.code==="ADAPTER_CONFLICT");
});
test("Anchor serialization is offline and preserves exact bytes",()=>{
    const bytes=Uint8Array.of(0,1,2,255);let called=0;
    const request=anchorRequest({requestId:"anchor",family:"v4-2",genesisHash:"explicit",candidate:{},runtimeBinding:{}},
        {serialize(options){assert.deepEqual(options,{requireAllSignatures:false,verifySignatures:false});called++;return bytes;}});
    assert.deepEqual(Buffer.from(request.replacementTransactionBase64,"base64"),Buffer.from(bytes));assert.equal(called,1);
});
test("CI does not count a guarded failure as a completed simulation",()=>{
    for(const outcome of ["NEEDS_INPUT","UNSUPPORTED","MISMATCH","CANCELLED","TIMEOUT","ERROR"])
        assert.throws(()=>assertCompleted({receipt:{outcome}}),e=>e.code==="REPLAY_NOT_COMPLETED");
    const completed={receipt:{outcome:"COMPLETED"},result:{file:"result.json",sha256:"explicit"}};
    assert.equal(assertCompleted(completed),completed);
});
