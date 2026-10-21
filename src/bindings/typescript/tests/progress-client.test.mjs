import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,writeFile,chmod,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {createHash} from "node:crypto";
import {Replay} from "../dist/index.js";

const digest=bytes=>createHash("sha256").update(bytes).digest("hex");
test("Replay.replay and Replay.simulate forward streaming callbacks to the pinned CLI",{skip:process.platform==="win32"},async()=>{
    const root=await mkdtemp(join(tmpdir(),"replay-progress-client-"));
    try {
        // Synthetic CLI protocol fixture: only admission errors, no replay/Bank substitute.
        const source=String.raw`#!/usr/bin/env node
const args=process.argv.slice(2);
if(args[0]==='doctor')process.stdout.write(JSON.stringify({schema:'svm-replay-capabilities/v1',outcome:'COMPLETED'}));
else if(args[0]==='validate')process.stdout.write(JSON.stringify({schema:'admission-fixture',outcome:'COMPLETED',limits:{maxOutputBytes:4096}}));
else {
    if(!args.includes('--progress'))throw new Error('progress flag missing');
    const event={schema:'svm-replay-progress/v1',origin:'native',sequence:1,kind:'phase',phase:'locating_transaction',elapsedMs:0,phaseDurationMs:0,sinceAdvanceMs:0,attempt:0};
    process.stderr.write(JSON.stringify(event)+'\n');
    setTimeout(()=>{
        process.stderr.write(JSON.stringify({...event,sequence:2,kind:'terminal',elapsedMs:100,phaseDurationMs:100,sinceAdvanceMs:100,outcome:'ERROR'})+'\n');
        process.stdout.write(JSON.stringify({schema:'svm-replay-cli-error/v1',outcome:'ERROR',error:{code:'SOURCE_CONFIGURATION',message:'offline fixture'}}));
        process.exitCode=1;
    },100);
}
`;
        await writeFile(join(root,"cli"),source);await chmod(join(root,"cli"),0o700);
        const manifest=JSON.stringify({schema:"svm-replay-bundle/v1",binary:"cli",files:[{path:"cli",sha256:digest(source),executable:true}]});
        const bundlePath=join(root,"bundle.json");await writeFile(bundlePath,manifest);
        const replay=await Replay.open({bundlePath,bundleSha256:digest(manifest)});
        for(const mode of ["replay","simulate"]){
            const events=[];let completed=false,first;
            const streamed=new Promise(resolve=>{first=resolve;});
            const onProgress=e=>{events.push(e);first();};
            const pending=(mode==="replay"?replay.replay({tx:"1".repeat(64),onProgress}):
                replay.simulate({request:{schema:"admission-fixture"},onProgress})).then(value=>{completed=true;return value;});
            await Promise.race([streamed,pending]);assert.equal(completed,false);
            const result=await pending;
            assert.equal(events.length,2);assert.equal(events.at(-1).outcome,"ERROR");
            assert.equal(result.receipt.error.code,"SOURCE_CONFIGURATION");assert.equal(result.stderr,"");
        }
    }finally{await rm(root,{recursive:true,force:false});}
});
