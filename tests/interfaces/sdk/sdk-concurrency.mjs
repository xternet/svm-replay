/** Real shared-cache variants; no mocked execution or expected-output injection. */
import assert from "node:assert/strict";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {pathToFileURL} from "node:url";
const [entry,bundle,pin,sourceRun,captures,integrationSummary,outArg]=process.argv.slice(2);
if(!outArg)throw new Error("usage: node sdk-concurrency.mjs SDK_ENTRY BUNDLE PIN SOURCE_RUN CAPTURES INTEGRATION_SUMMARY NEW_OUT");
const {Replay,readArtifact}=await import(pathToFileURL(resolve(entry)));
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const proof=JSON.parse(await readFile(integrationSummary,"utf8"));assert.equal(proof.status,"PASS");
const request=JSON.parse(await readFile(join(sourceRun,`${proof.caseId}.request.json`),"utf8"));
// Explicit new cache test budget, not a changed frozen-corpus acceptance limit.
request.limits.maxOutputBytes=256*1024*1024;
const replay=await Replay.open({bundlePath:resolve(bundle),bundleSha256:pin,dataDir:join(out,"data"),cache:"all",
    source:{path:join(captures,proof.caseId,"captured/manifest.json"),sha256:proof.sourceSha256}});
const parse=async r=>{assert.equal(r.receipt.outcome,"COMPLETED",JSON.stringify(r.receipt));
    return JSON.parse((await readArtifact(dirname(r.receipt.receiptPath),r.receipt.output,256*1024*1024)).toString());};
const start=performance.now(),cold=await replay.simulate({request});const baseline=await parse(cold);
const balance=((1n<<53n)+123456n).toString();
const variant={...request,requestId:"m18-concurrent-override",requestedAccountOverrides:[{pubkey:proof.anchor.payer,lamports:balance}]};
const [original,overridden]=await Promise.all([replay.simulate({request}),replay.simulate({request:variant})]);
const [originalOut,variantOut]=await Promise.all([parse(original),parse(overridden)]);
assert.deepEqual(originalOut,baseline);assert.deepEqual(variantOut.original,baseline.original);assert.deepEqual(variantOut.prefix,baseline.prefix);
assert.notEqual(original.receipt.cache.resultKey,overridden.receipt.cache.resultKey);
assert.equal(original.receipt.cache.resultCacheHit,true);assert.equal(original.receipt.cache.metrics.workerCalls,0);
assert.equal(overridden.receipt.cache.preparedCacheHit,true);assert.equal(overridden.receipt.cache.metrics.prefixTransactionsExecuted,0);
assert.equal(variantOut.requestedOverrides.status,"ok");
const changed=variantOut.requestedOverrides.accountTransitions.find(t=>t.pubkey===proof.anchor.payer);
assert.equal(changed.before.lamports,balance);
const summary={status:"PASS",bundleSha256:pin,caseId:proof.caseId,elapsedMs:performance.now()-start,
    receipts:[cold,original,overridden].map(r=>r.receipt.receiptPath),originalUnchanged:true,variantBeforeLamports:balance,
    resultHitWorkers:original.receipt.cache.metrics.workerCalls,variantPrefixExecuted:overridden.receipt.cache.metrics.prefixTransactionsExecuted};
await writeFile(join(out,"summary.json"),JSON.stringify(summary),{flag:"wx",mode:0o600});console.log(JSON.stringify(summary));
