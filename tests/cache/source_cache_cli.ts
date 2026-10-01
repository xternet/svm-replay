import { requiredPath } from "../harness/paths";
/** Three real outside-checkout source/worker runs sharing one persistent cache. */
import {createHash} from "node:crypto";
import {readFile,writeFile,mkdir,copyFile,chmod} from "node:fs/promises";
import {constants} from "node:fs";
import {join,resolve,dirname} from "node:path";
import {spawn} from "node:child_process";
import {checkSourceReceipt} from "../harness/receipt_artifacts";
const [binArg,outArg,priorArg,catalogArg]=process.argv.slice(2);
if(!binArg||!outArg||!priorArg||!catalogArg)throw new Error("usage: bun tests/cache/source_cache_cli.ts BINARY NEW_OUT SOURCE_RUN CATALOG_RUN");
const hash=(v:Uint8Array)=>createHash("sha256").update(v).digest("hex");
const read=async(p:string)=>JSON.parse(await readFile(p,"utf8"));
const save=(p:string,v:unknown)=>writeFile(p,JSON.stringify(v),{flag:"wx",mode:0o600});
const out=resolve(outArg),prior=resolve(priorArg),catalogRun=resolve(catalogArg);await mkdir(out,{mode:0o700});
const binary=join(out,"svm-replay"),bytes=await readFile(resolve(binArg));
await copyFile(resolve(binArg),binary,constants.COPYFILE_EXCL);await chmod(binary,0o700);
if(hash(await readFile(binary))!==hash(bytes))throw new Error("binary changed during install");
const row=(await read(join(prior,"summary.json"))).results[0],id=row.caseId;
const request=await read(join(prior,`${id}.request.json`));request.metadataPolicy="STRICT";
// Checkpoints contain prepared account images as well as outputs. The earlier
// 64MiB run correctly failed its explicit file bound; request256MiB for this test.
request.limits.maxOutputBytes=256*1024*1024;
await save(join(out,"request.json"),request);
const source=join(join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m11-historical-final-jWj9Uy"),id,"captured/manifest.json");
if(hash(await readFile(source))!==row.sourcePin)throw new Error("source changed");
const catalog=join(catalogRun,"catalog.json"),catalogPin=(await read(join(catalogRun,"summary.json"))).catalogSha256;
if(hash(await readFile(catalog))!==catalogPin)throw new Error("catalog changed");
const started=performance.now(),results=[];
for(const [name,mode] of [["cold","all"],["result-hit","all"],["prepared-hit","prepared"]]){
    const start=performance.now();
    const child=spawn(binary,["simulate","--request",join(out,"request.json"),"--catalog",catalog,"--catalog-sha256",catalogPin,
        "--source",source,"--source-sha256",row.sourcePin,"--cache",mode,"--data-dir",join(out,"data")],{cwd:out,stdio:["ignore","pipe","pipe"]});
    const stdout:Buffer[]=[],stderr:Buffer[]=[];child.stdout.on("data",v=>stdout.push(v));child.stderr.on("data",v=>stderr.push(v));
    const code=await new Promise<number|null>((done,fail)=>{child.once("error",fail);child.once("close",done);});
    await writeFile(join(out,`${name}.stdout.json`),Buffer.concat(stdout),{flag:"wx",mode:0o600});
    await writeFile(join(out,`${name}.stderr.txt`),Buffer.concat(stderr),{flag:"wx",mode:0o600});
    const receipt=JSON.parse(Buffer.concat(stdout).toString());
    await checkSourceReceipt(receipt);
    let outputSha256=null;
    if(receipt.outcome==="COMPLETED"){
        outputSha256=hash(await readFile(join(dirname(receipt.receiptPath),receipt.output.file)));
        if(outputSha256!==receipt.output.sha256)throw new Error("output hash differs");
    }
    const result={name,code,outcome:receipt.outcome,error:receipt.error,cache:receipt.cache,outputSha256,receiptPath:receipt.receiptPath,elapsedMs:performance.now()-start};
    results.push(result);console.log(JSON.stringify({name,code,outcome:result.outcome,error:result.error,cache:result.cache?.metrics,elapsedMs:result.elapsedMs}));
    if(code!==0)break;
}
const success=results.length===3&&results.every(r=>r.code===0&&r.outcome==="COMPLETED"&&r.outputSha256===results[0].outputSha256)
    &&results[0].cache.resultCacheHit===false&&results[0].cache.preparedCacheHit===false
    &&results[1].cache.resultCacheHit===true&&results[1].cache.metrics.workerCalls===0
    &&results[2].cache.preparedCacheHit===true&&results[2].cache.metrics.prefixTransactionsExecuted===0;
const summary={status:success?"PASS":"FAIL",caseId:id,elapsedMs:performance.now()-started,networkRequests:0,
    meaning:"Captured historical sources; raw/worker caches exercised. No live-provider throughput claim.",binarySha256:hash(bytes),results};
await save(join(out,"summary.json"),summary);console.log(JSON.stringify({status:summary.status,elapsedMs:summary.elapsedMs}));
if(!success)process.exitCode=1;
