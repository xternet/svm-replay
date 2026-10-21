import {test} from "node:test";
import assert from "node:assert/strict";
import {run} from "../dist/process.js";
import {signatureArgs} from "../dist/signature.js";

const options={timeoutMs:3000,maxBytes:4096,alchemy:false};
const event=(sequence,kind="advance",extra={})=>({schema:"svm-replay-progress/v1",origin:"native",sequence,kind,
    phase:"fetching_historical_accounts",elapsedMs:sequence*10,phaseDurationMs:sequence*10,sinceAdvanceMs:0,attempt:1,...extra});
const line=e=>JSON.stringify(e)+"\n";

test("SDK delivers fragmented progress while CLI is still running",async()=>{
    const events=[];let completed=false;
    let first;const streamed=new Promise(resolve=>{first=resolve;});
    const code=`const text=${JSON.stringify(line(event(1)))};process.stderr.write(text.slice(0,21));
        setTimeout(()=>process.stderr.write(text.slice(21)),10);
        setTimeout(()=>{process.stderr.write(${JSON.stringify(line(event(2,"terminal",{outcome:"COMPLETED"})))});process.stdout.write('{"receipt":true}');},200);`;
    const pending=run(process.execPath,["-e",code],{...options,onProgress:e=>{
        assert.equal(completed,false);events.push(e);first();
    }}).then(value=>{completed=true;return value;});
    await streamed;assert.equal(completed,false);
    const result=await pending;
    assert.equal(events.length,2);
    assert.equal(events[0].schema,"svm-replay-progress/v1");
    assert.equal(result.stdout,'{"receipt":true}');assert.equal(result.stderr,"");
});

test("progress volume does not consume bounded diagnostic storage",async()=>{
    const lines=Array.from({length:100},(_,i)=>line(event(i+1))).join("")+line(event(101,"terminal",{outcome:"COMPLETED"}));
    const events=[];
    const result=await run(process.execPath,["-e",`process.stderr.write(${JSON.stringify("note\n"+lines)});`],
        {...options,onProgress:e=>events.push(e)});
    assert.equal(result.stderr,"note\n");assert.equal(events.length,101);
    await assert.rejects(run(process.execPath,["-e","process.stderr.write('x'.repeat(8192))"],
        {...options,onProgress:()=>{}}),e=>e.code==="DIAGNOSTIC_LIMIT");
});

test("SDK timeout and cancellation produce one terminal event and reap child",async()=>{
    for(const mode of ["timeout","abort"]){
        const controller=new AbortController(),events=[];
        const code=`process.stderr.write(${JSON.stringify(line(event(1)))});setInterval(()=>{},1000);`;
        const pending=run(process.execPath,["-e",code],{...options,timeoutMs:mode==="timeout"?200:3000,
            signal:controller.signal,onProgress:e=>events.push(e)});
        const timer=mode==="abort"?setTimeout(()=>controller.abort(),200):undefined;
        try {await assert.rejects(pending,e=>e.code===(mode==="timeout"?"TIMEOUT":"CANCELLED"));}
        finally {if(timer!==undefined)clearTimeout(timer);}
        assert.equal(events.filter(e=>e.kind==="terminal").length,1);
        assert.equal(events.at(-1).outcome,mode==="timeout"?"TIMEOUT":"CANCELLED");
        assert.equal(events.at(-1).origin,"sdk");
    }
});

test("malformed versioned events and callback errors fail with bounded errors",async()=>{
    await assert.rejects(run(process.execPath,["-e",`process.stderr.write(${JSON.stringify(line({...event(1),elapsedMs:-1}))});`],
        {...options,onProgress:()=>{}}),e=>e.code==="HOST_PROTOCOL");
    await assert.rejects(run(process.execPath,["-e",`process.stderr.write(${JSON.stringify(line(event(1)))});`],
        {...options,onProgress:()=>{throw new Error("callback failed");}}),e=>e.code==="PROGRESS_CALLBACK");
});

test("signature progress is opt-in and default diagnostics are unchanged",async()=>{
    const tx="1".repeat(64);
    assert.ok(!signatureArgs({tx},"out").includes("--progress"));
    assert.ok(signatureArgs({tx,onProgress:()=>{}},"out").includes("--progress"));
    const text=line(event(1));
    assert.equal((await run(process.execPath,["-e",`process.stderr.write(${JSON.stringify(text)});`],options)).stderr,text);
});


test("unterminated malformed progress still ends with a typed error event",async()=>{
    const events=[];
    await assert.rejects(run(process.execPath,["-e",`process.stderr.write(${JSON.stringify(JSON.stringify({...event(1),phase:"wrong"}))});`],
        {...options,onProgress:e=>events.push(e)}),e=>e.code==="HOST_PROTOCOL");
    assert.equal(events.length,1);assert.equal(events[0].kind,"terminal");assert.equal(events[0].outcome,"ERROR");
});

test("pre-aborted execution emits cancellation without starting the CLI",async()=>{
    const events=[];
    await assert.rejects(run("/missing-binary",[],{...options,signal:AbortSignal.abort(),onProgress:e=>events.push(e)}),e=>e.code==="CANCELLED");
    assert.equal(events.length,1);assert.equal(events[0].outcome,"CANCELLED");
    assert.equal(events[0].phase,undefined);assert.equal(events[0].phaseDurationMs,undefined);
});


test("SDK timeout remains TIMEOUT when native cancellation cooperates",async()=>{
    const events=[];
    const code=`process.stderr.write(${JSON.stringify(line(event(1)))});
        process.on('SIGTERM',()=>{process.stderr.write(${JSON.stringify(line(event(2,"terminal",{outcome:"CANCELLED"})))});process.exit(1);});
        setInterval(()=>{},1000);`;
    await assert.rejects(run(process.execPath,["-e",code],{...options,timeoutMs:200,onProgress:e=>events.push(e)}),e=>e.code==="TIMEOUT");
    assert.equal(events.filter(e=>e.kind==="terminal").length,1);
    assert.equal(events.at(-1).outcome,"TIMEOUT");assert.equal(events.at(-1).origin,"sdk");
});


test("cached loads, growing totals and stalled heartbeats preserve native measurements",async()=>{
    const events=[];
    const stream=[event(1,"advance",{cacheHits:7,fetchedAccounts:0,downloadedBytes:0,
        earlierTransactionsTotal:{value:3,status:"provisional"}}),
        event(2,"advance",{earlierTransactionsTotal:{value:9,status:"provisional"}}),
        event(3,"heartbeat",{elapsedMs:30030,phaseDurationMs:30030,sinceAdvanceMs:30000,
            earlierTransactionsTotal:{value:9,status:"final",privatePayload:"discard"},privatePayload:"discard"}),
        event(4,"terminal",{elapsedMs:30040,outcome:"COMPLETED"})].map(line).join("");
    await run(process.execPath,["-e",`process.stderr.write(${JSON.stringify(stream)});`],{...options,onProgress:e=>events.push(e)});
    assert.equal(events[0].cacheHits,7);assert.equal(events[0].fetchedAccounts,0);
    assert.deepEqual(events[1].earlierTransactionsTotal,{value:9,status:"provisional"});
    assert.deepEqual(events[2].earlierTransactionsTotal,{value:9,status:"final"});
    assert.equal(events[2].sinceAdvanceMs,30000);assert.equal(events[2].outcome,undefined);
    assert.ok(events.every(e=>e.processedEarlierTransactions===undefined));
    assert.ok(!JSON.stringify(events).includes("privatePayload"));
});
