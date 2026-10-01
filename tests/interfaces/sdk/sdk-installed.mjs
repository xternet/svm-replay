/** Run using an npm-tarball installation, never source imports or prototype code. */
import assert from "node:assert/strict";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {join,resolve,dirname} from "node:path";
import {pathToFileURL} from "node:url";
import {createHash} from "node:crypto";
const [sdkPath,bundlePath,bundlePin,sourceRun,captureRoot,output,selection="all"]=process.argv.slice(2);
if(!sdkPath||!bundlePath||!bundlePin||!sourceRun||!captureRoot||!output)throw new Error("usage: sdk-installed.mjs SDK_ENTRY BUNDLE PIN SOURCE_RUN CAPTURES NEW_OUT [CASE_ID|all]");
const {Replay,readArtifact}=await import(pathToFileURL(resolve(sdkPath)).href);
const read=async path=>JSON.parse(await readFile(path,"utf8"));
const save=(path,value)=>writeFile(path,JSON.stringify(value),{flag:"wx",mode:0o600});
await mkdir(output,{mode:0o700});
const source=await read(join(sourceRun,"summary.json"));assert.equal(source.status,"PASS");
const rows=source.results.filter(r=>selection==="all"||r.caseId===selection);assert.ok(rows.length>0);
const results=[],started=performance.now();
for(const row of rows){
    const id=row.caseId,start=performance.now();
    const sourcePath=join(captureRoot,id,"captured/manifest.json");
    assert.equal(createHash("sha256").update(await readFile(sourcePath)).digest("hex"),row.sourcePin);
    const client=await Replay.open({bundlePath,bundleSha256:bundlePin,dataDir:join(output,"data"),source:{path:sourcePath,sha256:row.sourcePin}});
    const result=await client.simulate({request:{path:join(sourceRun,`${id}.request.json`)}});
    assert.equal(result.receipt.outcome,"COMPLETED",JSON.stringify(result.receipt.error));
    const original=await read(row.receiptPath);
    const expected=await readFile(join(dirname(row.receiptPath),original.output.file));
    const actual=await readArtifact(dirname(result.receipt.receiptPath),result.result,268435456);
    assert.deepEqual(JSON.parse(actual),JSON.parse(expected),"source replay output must match pre-M18 evidence");
    await save(join(output,`${id}.json`),result);
    const proof={caseId:id,status:"PASS",receiptPath:result.receipt.receiptPath,bytes:actual.length,elapsedMs:performance.now()-start};
    results.push(proof);console.log(JSON.stringify(proof));
}
await save(join(output,"summary.json"),{status:"PASS",runtime:process.versions,bundleSha256:bundlePin,cases:results,elapsedMs:performance.now()-started});
