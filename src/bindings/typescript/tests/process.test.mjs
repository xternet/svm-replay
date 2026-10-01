import {test} from "node:test";
import assert from "node:assert/strict";
import {run,environment} from "../dist/process.js";

const options={timeoutMs:3000,maxBytes:4096,alchemy:false};
test("bounded subprocess returns output and records exit failure",async()=>{
    const result=await run(process.execPath,["-e","process.stdout.write('hello');process.stderr.write('diagnostic');process.exitCode=7"],options);
    assert.deepEqual(result,{stdout:"hello",stderr:"diagnostic",exitCode:7,signal:null});
    await assert.rejects(run(process.execPath,["-e","process.stdout.write('x'.repeat(8192))"],options),e=>e.code==="DIAGNOSTIC_LIMIT");
    await assert.rejects(run("/missing-svm-replay-sdk-test-binary",[],options),e=>e.code==="PROCESS_ERROR");
});
test("deadline and cancellation wait for owned child termination",async()=>{
    const code="console.log(process.pid);setInterval(()=>{},1000)";
    for(const mode of ["timeout","abort"]){
        const controller=new AbortController();
        const pending=run(process.execPath,["-e",code],{...options,timeoutMs:mode==="timeout"?150:3000,signal:controller.signal});
        const timer=mode==="abort"?setTimeout(()=>controller.abort(),150):undefined;
        try {await assert.rejects(pending,e=>{
            assert.equal(e.code,mode==="timeout"?"TIMEOUT":"CANCELLED");
            const pid=Number(e.details.stdout.trim());assert.ok(pid>0);
            assert.throws(()=>process.kill(pid,0),e=>e.code==="ESRCH");return true;
        });}finally{if(timer!==undefined)clearTimeout(timer);}
    }
    await assert.rejects(run(process.execPath,["-e",code],{...options,signal:AbortSignal.abort()}),e=>e.code==="CANCELLED");
});
test("only the public Alchemy credential is forwarded when selected",()=>{
    const keys=["API_ALCHEMY","API_ALCHEMY_SOLANA_HIST","GITHUB_TOKEN_CLASSIC","OPENROUTER_API_KEY"];
    const old=keys.map(k=>process.env[k]);
    try{for(const key of keys)process.env[key]="test-only-sentinel";
        assert.equal(environment(false).API_ALCHEMY,undefined);
        assert.equal(environment(true).API_ALCHEMY,"test-only-sentinel");
        for(const key of keys.slice(1))assert.equal(environment(true)[key],undefined);
    }finally{keys.forEach((key,i)=>{if(old[i]===undefined)delete process.env[key];else process.env[key]=old[i];});}
});
