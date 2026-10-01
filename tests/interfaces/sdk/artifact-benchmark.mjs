import assert from "node:assert/strict";
import {readFile,writeFile} from "node:fs/promises";
import {resolve,dirname} from "node:path";
import {pathToFileURL} from "node:url";
const [entry,receiptPath,output,mode]=process.argv.slice(2);
if(!output||!["stream","read"].includes(mode))throw new Error("usage: node artifact-benchmark.mjs SDK_ENTRY RECEIPT NEW_OUTPUT stream|read");
const {verifyArtifact,readArtifact}=await import(pathToFileURL(resolve(entry)));
const receipt=JSON.parse(await readFile(receiptPath,"utf8"));assert.equal(receipt.outcome,"COMPLETED");
assert.equal(receipt.trace.exports.length,1);const artifact=receipt.trace.exports[0];
const start=performance.now(),before=process.memoryUsage().rss;let sampledPeak=before;
// getrusage high-water marks can include pre-exec launcher memory. Sample this
// Node process's actual RSS instead; this is an observed peak, not an exact bound.
const timer=setInterval(()=>{sampledPeak=Math.max(sampledPeak,process.memoryUsage().rss);},5);
let result;
try{result=mode==="stream"?await verifyArtifact(dirname(receiptPath),artifact,256*1024*1024)
    :await readArtifact(dirname(receiptPath),artifact,256*1024*1024);}
finally{clearInterval(timer);sampledPeak=Math.max(sampledPeak,process.memoryUsage().rss);}
const bytes=mode==="stream"?result.bytes:result.length;assert.equal(bytes,artifact.artifact.payload.bytes);
const summary={status:"PASS",mode,bytes,sha256:artifact.sha256,elapsedMs:performance.now()-start,
    sampledPeakRssBytes:sampledPeak,baselineRssBytes:before,
    note:"5ms RSS sampling in a separate Node process; same actual trace bytes, no JSON parsing. Not engine RSS or an exact memory bound."};
await writeFile(output,JSON.stringify(summary),{flag:"wx",mode:0o600});console.log(JSON.stringify(summary));
