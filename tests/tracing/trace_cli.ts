import { requiredPath } from "../harness/paths";
/** Real CLI observation export; retained fixtures are only reviewed inputs. */
import {createHash} from "node:crypto";
import {readFile,writeFile,mkdir,copyFile,chmod} from "node:fs/promises";
import {constants} from "node:fs";
import {join,resolve} from "node:path";
import {spawn} from "node:child_process";
import {frozenPolicy} from "../harness/frozen_policy";
const [binArg,outArg,identityArg]=process.argv.slice(2);
if(!binArg||!outArg||!identityArg)throw new Error("usage: bun tests/tracing/trace_cli.ts BINARY NEW_OUT PRIOR_TRACE_IDENTITY");
const hash=(v:Uint8Array)=>createHash("sha256").update(v).digest("hex");
const read=async(p:string)=>JSON.parse(await readFile(p,"utf8"));
const save=(p:string,v:unknown)=>writeFile(p,JSON.stringify(v),{flag:"wx",mode:0o600});
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const binary=join(out,"svm-replay"),bytes=await readFile(resolve(binArg));
await copyFile(resolve(binArg),binary,constants.COPYFILE_EXCL);await chmod(binary,0o700);
if(hash(await readFile(binary))!==hash(bytes))throw new Error("binary changed during install");
const identity=await read(resolve(identityArg));
if(hash(await readFile(identity.catalogPath))!==identity.catalogSha256)throw new Error("catalog changed");
const raw=await readFile(identity.requestPath);
if(hash(raw)!==identity.requestSha256)throw new Error("request changed");
const request=JSON.parse(raw.toString());
const corpus=await read(join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m12-sealed-historical-XmOnk2/historical/regression-inputs.json"));
const policy=await frozenPolicy(corpus.cases.find((r:any)=>r.candidate.id===request.candidate.id));
request.metadataPolicy=policy.metadataPolicy;
request.sourceEvidenceHashes.push(policy.evidenceSha256);
await save(join(out,"request.json"),request);
await save(join(out,"trace.json"),{capture:identity.policy,bounds:{registerRows:4,memoryRows:null,maxInvocations:128}});
const start=performance.now();
const child=spawn(binary,["simulate","--request",join(out,"request.json"),"--catalog",identity.catalogPath,"--catalog-sha256",identity.catalogSha256,
    "--trace",join(out,"trace.json"),"--data-dir",join(out,"data")],{cwd:out,stdio:["ignore","pipe","pipe"]});
const stdout:Buffer[]=[],stderr:Buffer[]=[];child.stdout.on("data",v=>stdout.push(v));child.stderr.on("data",v=>stderr.push(v));
const code=await new Promise<number|null>((done,fail)=>{child.once("error",fail);child.once("close",done);});
await writeFile(join(out,"stdout.json"),Buffer.concat(stdout),{flag:"wx",mode:0o600});
await writeFile(join(out,"stderr.txt"),Buffer.concat(stderr),{flag:"wx",mode:0o600});
const receipt=JSON.parse(Buffer.concat(stdout).toString()),exports=[];
if(code===0&&receipt.outcome==="COMPLETED"){
    for(const item of receipt.trace.exports){
        const payload=await readFile(join(receipt.receiptPath,"..",item.file));
        if(hash(payload)!==item.sha256||item.sha256!==item.artifact.payload.sha256||payload.length!==item.artifact.payload.bytes)throw new Error("export integrity failed");
        const events=JSON.parse(payload.toString());
        if(events.length!==item.artifact.payload.events)throw new Error("event count differs");
        exports.push({phase:item.phase,status:item.artifact.status,bytes:payload.length,events:events.length});
    }
}
const summary={status:code===0&&receipt.outcome==="COMPLETED"&&exports.length>0?"PASS":"FAIL",code,outcome:receipt.outcome,error:receipt.error,
    caseId:request.candidate.id,metadataPolicy:request.metadataPolicy,workerCalls:receipt.trace?.execution?.workerCalls,exports,
    elapsedMs:performance.now()-start,receiptPath:receipt.receiptPath,binarySha256:hash(bytes),networkRequests:0};
await save(join(out,"summary.json"),summary);console.log(JSON.stringify(summary));
if(summary.status!=="PASS")process.exitCode=1;
