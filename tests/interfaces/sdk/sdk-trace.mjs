/** Verify actual trace exports through the installed public SDK. */
import assert from "node:assert/strict";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {pathToFileURL} from "node:url";
const [entry,bundle,pin,traceCase,outArg]=process.argv.slice(2);
if(!outArg)throw new Error("usage: node sdk-trace.mjs SDK_ENTRY BUNDLE PIN PRIOR_TRACE_CASE NEW_OUT");
const {Replay,verifyArtifact}=await import(pathToFileURL(resolve(entry)));
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const replay=await Replay.open({bundlePath:resolve(bundle),bundleSha256:pin,dataDir:join(out,"data")});
const start=performance.now();
const result=await replay.simulate({request:{path:join(traceCase,"request.json")},trace:{path:join(traceCase,"trace.json")}});
assert.equal(result.receipt.outcome,"COMPLETED",JSON.stringify(result.receipt));
const previous=JSON.parse(await readFile(join(traceCase,"stdout.json"),"utf8"));
assert.equal(result.receipt.output.sha256,previous.output.sha256);
assert.equal(result.receipt.trace.exports.length,previous.trace.exports.length);
const artifacts=[];
for(let i=0;i<result.receipt.trace.exports.length;i++){
    const exported=result.receipt.trace.exports[i],expected=previous.trace.exports[i];
    assert.equal(exported.sha256,expected.sha256);assert.equal(exported.artifact.status,expected.artifact.status);
    const verified=await verifyArtifact(dirname(result.receipt.receiptPath),exported,256*1024*1024);
    artifacts.push({sha256:verified.sha256,bytes:verified.bytes,status:exported.artifact.status});
}
const summary={status:"PASS",receiptPath:result.receipt.receiptPath,bundleSha256:pin,artifacts,elapsedMs:performance.now()-start};
await writeFile(join(out,"summary.json"),JSON.stringify(summary),{flag:"wx",mode:0o600});console.log(JSON.stringify(summary));
