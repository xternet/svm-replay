import { requiredPath } from "../harness/paths";
/** Test-only captured source installer. All reconstruction/execution is native Rust. */
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { mkdir,readFile,writeFile,copyFile,chmod } from "node:fs/promises";
import { resolve,join } from "node:path";
import { spawn } from "node:child_process";
import { frozenPolicy } from "../harness/frozen_policy";
import { checkSourceReceipt } from "../harness/receipt_artifacts";
const [binaryArg,outArg,catalogRunArg,selection="m7-profile-03"]=process.argv.slice(2);
if(!binaryArg||!outArg||!catalogRunArg) throw new Error("usage: bun tests/historical/historical_source.ts BINARY NEW_OUTPUT_DIR PREPARED_RUN [case|six]");
const out=resolve(outArg),binary=join(out,"svm-replay"),catalogRun=resolve(catalogRunArg);
const digest=(value:Uint8Array|string)=>createHash("sha256").update(value).digest("hex");
const read=async(path:string)=>JSON.parse(await readFile(path,"utf8"));
const save=(path:string,value:unknown)=>writeFile(path,JSON.stringify(value),{flag:"wx",mode:0o600});
await mkdir(out,{mode:0o700});
const binaryBytes=await readFile(resolve(binaryArg));
await copyFile(resolve(binaryArg),binary,constants.COPYFILE_EXCL);await chmod(binary,0o700);
if(digest(await readFile(binary))!==digest(binaryBytes))throw new Error("CLI changed during install");
const catalog=join(catalogRun,"catalog.json"),catalogPin=(await read(join(catalogRun,"summary.json"))).catalogSha256;
if(digest(await readFile(catalog))!==catalogPin)throw new Error("catalog changed from recorded run pin");
const sourceRoot=join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m11-historical-final-jWj9Uy");
const cases=(await read(join(sourceRoot,"summary.json"))).results.map((r:any)=>r.caseId);
const corpus=(await read(join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m12-sealed-historical-XmOnk2/historical/regression-inputs.json"))).cases;
const installed=(await read(catalog)).workers;
const rows=selection==="six"?cases:cases.filter((id:string)=>id===selection);
if(!rows.length)throw new Error("unknown captured source case");
const results=[];const started=performance.now();
for(const id of rows){
    const start=performance.now(),entry=corpus.find((r:any)=>r.candidate.id===id),candidate=entry.candidate;
    const policy=await frozenPolicy(entry);
    const reference=await read(join(sourceRoot,id,"prepared.json"));
    const source=join(sourceRoot,id,"captured/manifest.json"),sourceBytes=await readFile(source),sourcePin=digest(sourceBytes);
    const identity=JSON.parse(sourceBytes.toString()).identity;
    const family=installed.find((w:any)=>w.executorSourceId===candidate.executorSourceId)?.family;
    if(!family)throw new Error("runtime not installed");
    const request=join(out,`${id}.request.json`);
    await save(request,{schema:"svm-replay-historical/v1",requestId:id,family,genesisHash:identity.genesisHash,
        candidate,runtimeBinding:reference.runtime.binding,replacementTransactionBase64:null,requestedAccountOverrides:null,
        bankInputs:[],metadataPolicy:policy.metadataPolicy,limits:{timeoutMs:300000,maxOutputBytes:67108864,maxDiagnosticBytes:1048576}});
    const child=spawn(binary,["simulate","--request",request,"--catalog",catalog,"--catalog-sha256",catalogPin,
        "--source",source,"--source-sha256",sourcePin,"--data-dir",join(out,"data")],{cwd:out,stdio:["ignore","pipe","pipe"]});
    const stdout:Buffer[]=[],stderr:Buffer[]=[];child.stdout.on("data",v=>stdout.push(v));child.stderr.on("data",v=>stderr.push(v));
    const code=await new Promise<number|null>((done,fail)=>{child.once("error",fail);child.once("close",done);});
    await writeFile(join(out,`${id}.stdout.json`),Buffer.concat(stdout),{flag:"wx",mode:0o600});
    await writeFile(join(out,`${id}.stderr.txt`),Buffer.concat(stderr),{flag:"wx",mode:0o600});
    const receipt=JSON.parse(Buffer.concat(stdout).toString());
    await checkSourceReceipt(receipt);
    const row={caseId:id,code,outcome:receipt.outcome,error:receipt.error,verification:receipt.verification,
        reconstructedFromSources:receipt.reconstructedFromSources,receiptPath:receipt.receiptPath,sourcePin,policyEvidenceSha256:policy.evidenceSha256,elapsedMs:performance.now()-start};
    results.push(row);console.log(JSON.stringify(row));
    if(digest(await readFile(source))!==sourcePin)throw new Error("source manifest changed during run");
}
const summary={schema:"svm-replay-m17-source-parity/v1",count:results.length,elapsedMs:performance.now()-started,
    status:results.every(r=>r.code===0&&r.outcome==="COMPLETED"&&r.reconstructedFromSources===true)?"PASS":"FAIL",
    binarySha256:digest(binaryBytes),catalogPin,networkRequests:0,results};
await save(join(out,"summary.json"),summary);console.log(JSON.stringify({status:summary.status,count:summary.count,elapsedMs:summary.elapsedMs}));
if(summary.status!=="PASS")process.exitCode=1;
