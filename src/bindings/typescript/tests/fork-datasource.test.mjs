import {test} from "node:test";
import assert from "node:assert/strict";
import {serveBoundary,MAINNET_GENESIS} from "../dist/adapters/fork/datasource.js";

const FALLBACK_PHRASES=["not supported","unsupported","unavailable","method blocked","invalid request","is blocked","if you need this method","410"];
const PROGRAM="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",ABSENT="So11111111111111111111111111111111111111112";
const OUTSIDE="SysvarRent111111111111111111111111111111111",U64_MAX="18446744073709551615";
const CHANGED="7wTqPbncV3HiwC7RckPo8SAC6bATAYtxQau5eekbnmqe",CLOCK="SysvarC1ock11111111111111111111111111111111";
const sysvar=(pubkey,bytes)=>({pubkey,lamports:"1169280",owner:"Sysvar1111111111111111111111111111111111111",executable:false,rentEpoch:"0",
    dataBase64:Buffer.from(bytes).toString("base64")});
const boundary=()=>({signature:"5qg9drHYZVtxGPj9f8LGZcdCB9mFydTgcMQ8d4Z2AQK5WwBvSwWSc2yEiu5mouwRsXbF5YA5ztxrsRujA1kZxWEF",
    features:[],targetSlot:422490991,epoch:978,blockHeight:400539222,absent:new Set([ABSENT]),unavailable:new Set([CHANGED]),clock:sysvar(CLOCK,[9]),
    accounts:new Map([[PROGRAM,{pubkey:PROGRAM,lamports:"934087680",owner:"BPFLoader2111111111111111111111111111111111",
        executable:true,rentEpoch:U64_MAX,dataBase64:Buffer.from([1,2,3,4]).toString("base64")}],[CLOCK,sysvar(CLOCK,[9])]])});
async function rpc(url,body){
    const response=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(body)});
    return response.text();
}
async function served(run){const source=await serveBoundary(boundary());try{await run(source);}finally{await source.close();}}
const call=(method,params=[])=>({jsonrpc:"2.0",id:7,method,params});

test("serves byte-exact historical images with exact u64 values on localhost",()=>served(async source=>{
    assert.match(source.url,/^http:\/\/127\.0\.0\.1:\d+$/);
    // Surfpool (Rust RpcClient) sends dataSlice: null and asks for base64+zstd; base64 is a valid answer.
    const text=await rpc(source.url,call("getAccountInfo",[PROGRAM,{encoding:"base64+zstd",dataSlice:null}]));
    // rentEpoch must stay u64::MAX in the JSON text; JSON.parse would round it.
    assert.ok(text.includes(`"rentEpoch":${U64_MAX}`));
    const {result}=JSON.parse(text);
    assert.equal(result.context.slot,422490991);
    assert.deepEqual(result.value.data,[Buffer.from([1,2,3,4]).toString("base64"),"base64"]);
    assert.equal(result.value.lamports,934087680);assert.equal(result.value.executable,true);assert.equal(result.value.space,4);
}));
test("accounts outside the boundary fail explicitly, without Surfpool's silent-fallback phrases",()=>served(async source=>{
    for(const request of [call("getAccountInfo",[OUTSIDE]),call("getMultipleAccounts",[[PROGRAM,OUTSIDE]]),call("getProgramAccounts",[PROGRAM])]){
        const {error,result}=JSON.parse(await rpc(source.url,request));
        assert.equal(result,undefined);assert.ok(error.code<0);
        for(const phrase of FALLBACK_PHRASES)assert.ok(!error.message.toLowerCase().includes(phrase),`${request.method}: "${phrase}"`);
    }
    assert.deepEqual([...source.misses],[OUTSIDE]);
}));
test("only proven absences and declared-new keys are served as absent",()=>served(async source=>{
    assert.equal(JSON.parse(await rpc(source.url,call("getAccountInfo",[ABSENT]))).result.value,null);
    assert.ok(JSON.parse(await rpc(source.url,call("getAccountInfo",[OUTSIDE]))).error);
    source.fresh.add(OUTSIDE);
    const {result}=JSON.parse(await rpc(source.url,call("getMultipleAccounts",[[PROGRAM,ABSENT,OUTSIDE]])));
    assert.equal(result.value.length,3);assert.equal(result.value[1],null);assert.equal(result.value[2],null);
}));
test("epoch, schedule and genesis describe the historical boundary",()=>served(async source=>{
    const [info,schedule,genesis]=JSON.parse(await rpc(source.url,[call("getEpochInfo"),call("getEpochSchedule"),call("getGenesisHash")]));
    assert.deepEqual(info.result,{absoluteSlot:422490991,blockHeight:400539222,epoch:978,slotIndex:422490991-978*432000,slotsInEpoch:432000,transactionCount:null});
    assert.equal(schedule.result.slotsPerEpoch,432000);assert.equal(schedule.result.warmup,false);
    assert.equal(genesis.result,MAINNET_GENESIS);
}));
test("dataSlice and malformed input are handled explicitly",()=>served(async source=>{
    const sliced=JSON.parse(await rpc(source.url,call("getAccountInfo",[PROGRAM,{dataSlice:{offset:1,length:2}}])));
    assert.deepEqual(Buffer.from(sliced.result.value.data[0],"base64"),Buffer.from([2,3]));assert.equal(sliced.result.value.space,4);
    assert.equal(JSON.parse(await rpc(source.url,call("getAccountInfo",["not-an-address"]))).error.code,-32602);
    const garbage=await fetch(source.url,{method:"POST",body:"{"});
    assert.equal(JSON.parse(await garbage.text()).error.code,-32700);
}));
test("runtime-only instruction sysvar is absent; arbitrary derived addresses need historical proof",()=>served(async source=>{
    assert.equal(JSON.parse(await rpc(source.url,call("getAccountInfo",["Sysvar1nstructions1111111111111111111111111"]))).result.value,null);
    assert.equal(JSON.parse(await rpc(source.url,call("getAccountInfo",["3gvYRKWyXRR9xKWe1ZjPhLY5ZJRN7KDB4rFZFGoJfFk2"]))).error.code,-32010);
}));
test("consumer-specific configuration and alignment methods have no historical source",()=>served(async source=>{
    for(const method of ["svmReplayFork_surfnetConfig","svmReplayFork_alignment"])
        assert.equal(JSON.parse(await rpc(source.url,call(method))).error.code,-32601);
}));
test("a declared new program or wallet owns nothing historically; any other scan fails explicitly",()=>served(async source=>{
    const NEW="2TnwUoEbP8RKeNP3uimWaW78ZEdmJwCRMRsTUmZkMRmh";source.fresh.add(NEW);
    assert.deepEqual(JSON.parse(await rpc(source.url,call("getProgramAccounts",[NEW,{encoding:"base64"}]))).result,[]);
    assert.deepEqual(JSON.parse(await rpc(source.url,call("getProgramAccounts",[NEW,{withContext:true}]))).result.value,[]);
    assert.deepEqual(JSON.parse(await rpc(source.url,call("getTokenAccountsByOwner",[NEW,{programId:PROGRAM}]))).result.value,[]);
    for(const key of [PROGRAM,ABSENT,OUTSIDE])assert.equal(JSON.parse(await rpc(source.url,call("getProgramAccounts",[key]))).error.code,-32601);
    assert.deepEqual(JSON.parse(await rpc(source.url,call("svmReplayFork_status"))).result.assumedAbsent,[]);
}));
test("development mode assumes an unknown program owns nothing, and reports it",async()=>{
    const source=await serveBoundary(boundary(),0,true);
    try {
        assert.deepEqual(JSON.parse(await rpc(source.url,call("getProgramAccounts",[OUTSIDE]))).result,[]);
        assert.equal(JSON.parse(await rpc(source.url,call("getProgramAccounts",[PROGRAM]))).error.code,-32601);
        assert.deepEqual(JSON.parse(await rpc(source.url,call("svmReplayFork_status"))).result.assumedAbsent,[OUTSIDE]);
    } finally {await source.close();}
});
test("accounts changed earlier in the slot are an explicit error, never an older image",()=>served(async source=>{
    for(const request of [call("getAccountInfo",[CHANGED]),call("getMultipleAccounts",[[PROGRAM,CHANGED]])]){
        const {error,result}=JSON.parse(await rpc(source.url,request));
        assert.equal(result,undefined);assert.equal(error.code,-32011);assert.ok(error.message.includes(CHANGED));
        for(const phrase of FALLBACK_PHRASES)assert.ok(!error.message.toLowerCase().includes(phrase),phrase);
    }
    assert.deepEqual([...source.misses],[CHANGED]);
}));
test("development mode serves unknown accounts as absent and reports every one",async()=>{
    const source=await serveBoundary(boundary(),0,true);
    try {
        assert.equal(JSON.parse(await rpc(source.url,call("getAccountInfo",[OUTSIDE]))).result.value,null);
        const {result}=JSON.parse(await rpc(source.url,call("svmReplayFork_status")));
        assert.deepEqual(result.assumedAbsent,[OUTSIDE]);assert.deepEqual(result.misses,[]);
    } finally {await source.close();}
});
test("an idle pooled connection stays usable, as Surfpool's HTTP client expects",()=>served(async source=>{
    const {request,Agent}=await import("node:http");
    const agent=new Agent({keepAlive:true,maxSockets:1}),url=new URL(source.url);
    const post=()=>new Promise((done,fail)=>{const req=request({host:url.hostname,port:url.port,method:"POST",agent,headers:{"content-type":"application/json"}},
        res=>{res.resume();res.on("end",()=>done(req.reusedSocket));});req.on("error",fail);req.end(JSON.stringify(call("getGenesisHash")));});
    try {
        assert.equal(await post(),false);
        await new Promise(r=>setTimeout(r,6500));  // Node closes idle connections after 5 s plus a 1 s buffer by default
        assert.equal(await post(),true,"the server must not have closed the idle connection");
    } finally {agent.destroy();}
}));
