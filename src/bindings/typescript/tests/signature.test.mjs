import {test} from "node:test";
import assert from "node:assert/strict";
import {signatureArgs} from "../dist/signature.js";
import {withHistoricalReplay,anchorSignature} from "../dist/adapters/index.js";
test("collection all is forwarded literally and old trace rejects",()=>{
    const args=signatureArgs({tx:"1".repeat(64),collect:"all"},"output");
    assert.equal(args[args.indexOf("--collect")+1],"all");
    assert.throws(()=>signatureArgs({tx:"1".repeat(64),trace:true},"output"),e=>e.code==="INVALID_REQUEST");
});
test("signature API forwards controls without recreating historical logic",()=>{
    const args=signatureArgs({tx:"1".repeat(64),collect:{calls:true},fields:["status"],noCache:true,timeoutMs:1500,maxRequests:12},"output");
    assert.ok(args.includes("--tx"));assert.ok(args.includes("--collect"));assert.ok(args.includes("--no-cache"));
    assert.equal(args[args.indexOf("--timeout")+1],"2");
    assert.throws(()=>signatureArgs({tx:"bad"},"output"),e=>e.code==="INVALID_REQUEST");
});
test("Surfpool companion accepts a signature without touching current state",async()=>{
    const calls=[];
    const host=withHistoricalReplay({}, {replay:async input=>{calls.push(input);return {receipt:{outcome:"COMPLETED"}};}});
    const request={tx:"1".repeat(64)};await host.simulateTransactionAt(request);assert.deepEqual(calls,[request]);
    const replacement=anchorSignature(request.tx,{serialize:()=>Uint8Array.of(1,2)});
    assert.equal(replacement.tx,request.tx);assert.equal(replacement.replace.transactionBase64,"AQI=");
});
