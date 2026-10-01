/** Cancel only after this installed SDK's real pinned SVM worker is live. */
import assert from "node:assert/strict";
import {readFile,writeFile,mkdir,readdir} from "node:fs/promises";
import {resolve,join} from "node:path";
import {pathToFileURL} from "node:url";
const [entry,bundle,pin,request,source,sourcePin,outArg]=process.argv.slice(2);
if(!outArg)throw new Error("usage: node sdk-cancel.mjs SDK_ENTRY BUNDLE PIN REQUEST SOURCE SOURCE_PIN NEW_OUT");
const {Replay}=await import(pathToFileURL(resolve(entry)));
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const replay=await Replay.open({bundlePath:resolve(bundle),bundleSha256:pin,dataDir:join(out,"data"),source:{path:resolve(source),sha256:sourcePin}});
const controller=new AbortController(),seen=new Set();let done=false,observationError,depth=0;
async function children(pid,level=0){
    let tasks;
    try{tasks=await readdir(`/proc/${pid}/task`);}catch(e){if(e.code==="ENOENT"){console.error(`observer: process ${pid} already exited`);return;}throw e;}
    for(const tid of tasks){
        let value;
        try{value=await readFile(`/proc/${pid}/task/${tid}/children`,"utf8");}catch(e){if(e.code==="ENOENT"){console.error(`observer: thread ${pid}/${tid} already exited`);continue;}throw e;}
        for(const item of value.trim().split(/\s+/).filter(Boolean)){
            const child=Number(item);assert.ok(Number.isSafeInteger(child)&&child>0);seen.add(child);depth=Math.max(depth,level+1);
            await children(child,level+1);
        }
    }
}
const observe=(async()=>{
    while(!done){
        try{await children(process.pid);if(depth>=2){controller.abort();return;}}
        catch(e){observationError=e;controller.abort();return;}
        await new Promise(r=>setTimeout(r,100));
    }
})();
let failure;const start=performance.now();
try{await replay.simulate({request:{path:resolve(request)},signal:controller.signal,timeoutMs:60000});}
catch(e){failure=e;}
finally{done=true;await observe;}
if(observationError!==undefined)throw observationError;
assert.equal(failure?.code,"CANCELLED");assert.ok(depth>=2,"must observe a real CLI+worker tree before cancellation");
for(const pid of seen)assert.throws(()=>process.kill(pid,0),e=>e.code==="ESRCH",`owned pid ${pid} survived SDK closure`);
const receipt=JSON.parse(failure.details.stdout);assert.equal(receipt.outcome,"CANCELLED");
const stored=JSON.parse(await readFile(receipt.receiptPath,"utf8"));assert.equal(stored.outcome,"CANCELLED");
const summary={status:"PASS",knownPids:[...seen],unreaped:[],outcome:receipt.outcome,receiptPath:receipt.receiptPath,
    binarySha256:receipt.implementationSha256,bundleSha256:pin,elapsedMs:performance.now()-start};
await writeFile(join(out,"summary.json"),JSON.stringify(summary),{flag:"wx",mode:0o600});console.log(JSON.stringify(summary));
