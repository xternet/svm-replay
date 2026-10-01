import { requiredPath } from "../harness/paths";
/** Revalidate actual successful debugger receipts; never rewrite failed tranches. */
import {strict as assert} from "node:assert";
import {createHash} from "node:crypto";
import {readFile,writeFile} from "node:fs/promises";
import {dirname,join,resolve} from "node:path";
const [outputArg,firstArg,settledArg,newestArg]=process.argv.slice(2);
if(!outputArg||!firstArg||!settledArg||!newestArg)throw Error("usage: bun tests/debugging/debug_qualification_summary.ts NEW_SUMMARY FIRST_TRANCHE SETTLED_TRANCHE NEWEST_RUN");
const hash=(bytes:Uint8Array)=>createHash("sha256").update(bytes).digest("hex");
const inputPins:Record<string,string>={};
async function bound(path:string,pin?:string){const bytes=await readFile(path),actual=hash(bytes);if(pin!==undefined)assert.equal(actual,pin,path);inputPins[path]=actual;return bytes;}
const manifestPath=join(requiredPath("SVM_REPLAY_TEST_PROTOTYPE"), "poc/m13-tracing/workers/full-corpus/workers-manifest.json");
const manifest=JSON.parse((await bound(manifestPath)).toString());
async function check(row:any){
    const family=row.family,base=resolve(["v2-3","v3-0","v3-1"].includes(family)?firstArg:settledArg,family);
    const run=family==="v4-2"?resolve(newestArg):join(base,"run"),summaryPath=join(run,"summary.json");
    const summary=JSON.parse((await bound(summaryPath)).toString());assert.equal(summary.status,"PASS",summaryPath);
    assert.equal(summary.code,0);assert.equal(summary.outcome,"COMPLETED");assert.deepEqual(summary.unreaped,[]);
    assert.equal(summary.checks.workerTreeReaped,true);assert.equal(summary.checks.pcAdvanced,true);assert.equal(summary.checks.eventsAndExportsHashBound,true);
    const identity=JSON.parse((await bound(join(base,"identity.json"),summary.identitySha256)).toString());
    const request=JSON.parse((await bound(identity.requestPath,summary.requestSha256)).toString());assert.equal(request.candidate.id,summary.caseId);
    const catalog=JSON.parse((await bound(identity.catalogPath,summary.catalogSha256)).toString());
    assert.equal(catalog.workers.length,1);assert.equal(catalog.captureWorkers.length,1);
    const reference=catalog.workers[0],capture=catalog.captureWorkers[0];assert.equal(reference.family,family);assert.equal(capture.worker.family,family);
    assert.equal(reference.sha256,row.referenceSha256);assert.equal(capture.worker.sha256,row.binarySha256);assert.equal(capture.gateSha256,row.gateSha256);
    await bound(join(dirname(identity.catalogPath),reference.file),row.referenceSha256);
    await bound(join(dirname(identity.catalogPath),capture.worker.file),row.binarySha256);await bound(row.gate,row.gateSha256);
    await bound(join(run,"svm-replay"),summary.binarySha256);
    const configured=JSON.parse((await bound(join(run,"trace.json"))).toString());assert.deepEqual(configured,{capture:identity.policy,bounds:identity.bounds});
    const receipt=JSON.parse((await bound(summary.receiptPath)).toString());
    assert.equal(receipt.outcome,"COMPLETED");assert.equal(receipt.controlVerification.status,"PASS");assert.equal(receipt.verification.status,"PASS");
    assert.equal(receipt.currentStateFallback,false);assert.equal(receipt.catalogSha256,summary.catalogSha256);
    const execution=receipt.debug.execution;assert.equal(execution.status,"PASS");assert.equal(execution.implementationSha256,summary.binarySha256);
    assert.equal(execution.referenceSha256,row.referenceSha256);assert.equal(execution.captureWorkerSha256,row.binarySha256);assert.equal(execution.gateSha256,row.gateSha256);
    const live=JSON.parse((await bound(join(run,"observed-events.json"))).toString());assert.equal(live.length,summary.eventCount);
    const storedEvents=JSON.parse((await bound(join(dirname(summary.receiptPath),receipt.debug.events.file),receipt.debug.events.sha256)).toString());
    assert.deepEqual(storedEvents,live.filter((event:any)=>!["transaction-complete","session-complete"].includes(event.kind)));
    const complete=live.filter((event:any)=>event.kind==="session-complete");assert.equal(complete.length,1);assert.deepEqual(complete[0].receipt,execution);
    const exports=[];
    for(const item of receipt.debug.exports){const bytes=await bound(join(dirname(summary.receiptPath),item.file),item.sha256);
        assert.equal(item.sha256,item.artifact.payload.sha256);assert.equal(bytes.length,item.artifact.payload.bytes);assert.equal(JSON.parse(bytes.toString()).length,item.artifact.payload.events);
        exports.push({phase:item.phase,...item.artifact});}
    assert.ok(exports.length>0);await bound(join(dirname(summary.receiptPath),receipt.output.file),receipt.output.sha256);
    if(family==="v4-2"){
        assert.equal(summary.mode,"accounts");assert.equal(summary.accountInspection.status,"PASS");
        assert.equal(request.limits.timeoutMs,180000);assert.equal(identity.policy.limits.timeoutMs,60000);
        assert.ok(Array.isArray(execution.finalizedInventory)&&execution.finalizedInventory.length>0);
    }
    return {family,caseId:summary.caseId,status:"PASS",summaryPath,receiptPath:summary.receiptPath,binarySha256:summary.binarySha256,
        referenceSha256:row.referenceSha256,captureSha256:row.binarySha256,gateSha256:row.gateSha256,elapsedMs:summary.elapsedMs,
        policy:identity.policy,bounds:identity.bounds,requestTimeoutMs:request.limits.timeoutMs,exports,
        accountInspection:summary.accountInspection,finalizedInventory:execution.finalizedInventory};
}
const cases=[];
for(const row of manifest.workers){cases.push(await check(row));Bun.gc(true);}
assert.equal(new Set(cases.map(row=>row.family)).size,manifest.workers.length);
const summary={schema:"svm-replay-m17-debug-six-qualification/v1",status:"PASS",scope:"retained finite cases across explicitly pinned M17 releases; newest family targeted rerun, not all cases on final binary",
    cases,inputPins,networkRequests:0};
await writeFile(resolve(outputArg),JSON.stringify(summary),{flag:"wx",mode:0o600});
console.log(JSON.stringify({status:summary.status,cases:cases.map(row=>({family:row.family,caseId:row.caseId,binarySha256:row.binarySha256}))}));
