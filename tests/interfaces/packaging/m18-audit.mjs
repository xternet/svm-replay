/** Final finite-scope evidence audit. Requires completed runs, never synthesizes them. */
import assert from "node:assert/strict";
import {readFile,writeFile} from "node:fs/promises";
import {join,resolve,dirname} from "node:path";
import {createHash} from "node:crypto";
import {pathToFileURL} from "node:url";
const [rootArg,binaryArg,outputArg]=process.argv.slice(2);
if(!binaryArg)throw new Error("usage: node m18-audit.mjs M18_ARTIFACT_ROOT FINAL_BINARY [NEW_SEAL_PATH]");
const root=resolve(rootArg),hash=b=>createHash("sha256").update(b).digest("hex");
const read=async p=>JSON.parse(await readFile(p,"utf8"));
const {verifyArtifact}=await import(pathToFileURL(join(root,"client-qualified/node_modules/@svm-replay/sdk/dist/index.js")));
const binarySha256=hash(await readFile(binaryArg)),summaries={},pins={};
for(const name of ["isolated-types","large-artifact-stream-sampled","large-artifact-read-sampled"]){
    const bytes=await readFile(join(root,`${name}.json`));assert.equal(JSON.parse(bytes).status,"PASS");pins[name]=hash(bytes);
}
for(const name of ["replay-47","trace-47","node-qualified","bun-qualified","debug-six","cache","readonly",
    "sdk-trace","sdk-cancel","sdk-concurrency","sdk-rejections","integrations-second","rust-consumer"]){
    const path=join(root,name,"summary.json"),bytes=await readFile(path),value=JSON.parse(bytes);
    assert.equal(value.status,"PASS",name);summaries[name]=value;pins[name]=hash(bytes);
}
const policies={};
assert.equal(summaries["replay-47"].count,47);assert.equal(summaries["trace-47"].requestedCases,47);
for(const row of summaries["replay-47"].results)policies[row.metadataPolicy]=(policies[row.metadataPolicy]===undefined?0:policies[row.metadataPolicy])+1;
assert.deepEqual(policies,{STRICT:41,ARCHIVED_COMPUTE_METER_WARNING:6});
for(const name of ["node-qualified","bun-qualified","debug-six"])assert.equal(summaries[name].cases.length,6);
const receipts=new Map();
function add(path,host=binarySha256,outcome="COMPLETED"){assert.equal(typeof path,"string");receipts.set(path,{host,outcome});}
for(const name of ["replay-47","trace-47","cache"])for(const row of summaries[name].results)add(row.receiptPath);
for(const name of ["node-qualified","bun-qualified","debug-six"])for(const row of summaries[name].cases)add(row.receiptPath);
for(const name of ["readonly","sdk-trace"])add(summaries[name].receiptPath);
for(const path of summaries["sdk-concurrency"].receipts)add(path);
for(const name of ["surfpool","anchor"])add(summaries["integrations-second"][name].receiptPath);
add(summaries["sdk-cancel"].receiptPath,binarySha256,"CANCELLED");
add(summaries["rust-consumer"].receiptPath,summaries["rust-consumer"].hostSha256);
let files=0,bytes=0,truncated=0;
for(const [path,expected] of receipts){
    const receipt=await read(path);assert.equal(receipt.outcome,expected.outcome);
    assert.equal(receipt.implementationSha256,expected.host);assert.equal(receipt.processOwnerSha256,binarySha256);
    assert.equal(receipt.currentStateFallback,false);
    if(expected.outcome!=="COMPLETED")continue;
    assert.equal(receipt.controlVerification.status,"PASS");assert.equal(receipt.verification.status,"PASS");
    const exports=[receipt.output,...(receipt.trace===undefined?[]:receipt.trace.exports),...(receipt.debug===undefined?[]:receipt.debug.exports)];
    for(const artifact of exports){const checked=await verifyArtifact(dirname(path),artifact,256*1024*1024);files++;bytes+=checked.bytes;
        if(artifact.artifact?.status==="TRUNCATED")truncated++;}
}
const bundleBytes=await readFile(join(root,"install-final/bundle.json")),bundle=JSON.parse(bundleBytes);
for(const entry of bundle.files){const checked=await verifyArtifact(join(root,"install-final"),{file:entry.path,sha256:entry.sha256},512*1024*1024);assert.equal(checked.bytes,entry.bytes);}
assert.equal(bundle.files.find(f=>f.path===bundle.binary).sha256,binarySha256);
const log=await readFile(join(root,"ordinary-final.log"),"utf8");let rustPassed=0,rustIgnored=0;
for(const match of log.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/g)){
    assert.equal(Number(match[2]),0);rustPassed+=Number(match[1]);rustIgnored+=Number(match[3]);}
assert.equal(rustPassed,304);
const sdkBytes=await readFile(join(root,"package-qualified/svm-replay-sdk-0.1.0.tgz"));
const summary={schema:"svm-replay-m18-qualification/v1",status:"PASS",completedAt:new Date().toISOString(),
    binarySha256,bundleSha256:hash(bundleBytes),sdkTarballSha256:hash(sdkBytes),sdkTarballBytes:sdkBytes.length,
    rustPassed,rustIgnored,policies,receiptsChecked:receipts.size,artifactsChecked:files,artifactBytesChecked:bytes,
    explicitlyTruncatedExports:truncated,summaryPins:pins,
    scope:"Captured/prepared finite benchmark, Linux x86_64 GNU libc >=2.38. No live-provider throughput or universal/cross-platform claim; no publication."};
await writeFile(outputArg===undefined?join(root,"qualification.json"):resolve(outputArg),JSON.stringify(summary),{flag:"wx",mode:0o600});console.log(JSON.stringify(summary));
