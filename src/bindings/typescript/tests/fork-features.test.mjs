import {test} from "node:test";
import assert from "node:assert/strict";
import {serveBoundary} from "../dist/adapters/fork/datasource.js";
import {base58Encode} from "../dist/adapters/fork/accounts.js";
import {nativeFeatures} from "./native-boundary.mjs";

const FEATURE="Feature111111111111111111111111111111111111",RENT="SysvarRent111111111111111111111111111111111";
const SYSVAR="Sysvar1111111111111111111111111111111111111",MAX="18446744073709551615";
const ids=[21,22].map(n=>base58Encode(Buffer.alloc(32,n)));
function boundary(rate=7000n,threshold=1.5){
    const bytes=Buffer.alloc(17);bytes.writeBigUInt64LE(rate);bytes.writeDoubleLE(threshold,8);bytes[16]=50;
    return {signature:"feature-test",targetSlot:300,features:[{id:ids[0],activationSlot:0},{id:ids[1],activationSlot:299}],
        accounts:new Map([[RENT,{pubkey:RENT,owner:SYSVAR,executable:false,lamports:"1",rentEpoch:MAX,dataBase64:bytes.toString("base64")}]]),
        absent:new Set(),unavailable:new Set()};
}
async function served(b,run,allow=false){Object.assign(b,await nativeFeatures(b));const source=await serveBoundary(b,0,allow);try{await run(source);}finally{await source.close();}}
async function rpc(source,method,params=[]){
    const r=await fetch(source.url,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})});
    const text=await r.text();return {text,...JSON.parse(text)};
}
test("feature scans and direct reads use binding slots and historical Rent, reported as derived",()=>served(boundary(),async s=>{
    const scan=await rpc(s,"getProgramAccounts",[FEATURE,{encoding:"base64"}]);
    assert.equal(scan.error,undefined);assert.deepEqual(scan.result.map(a=>a.pubkey),ids);
    for(let i=0;i<ids.length;i++){
        const a=scan.result[i].account,data=Buffer.from(a.data[0],"base64");
        assert.equal(data.length,9);assert.equal(data[0],1);assert.equal(data.readBigUInt64LE(1),BigInt(i===0?0:299));
        assert.equal(a.lamports,Math.trunc((128+9)*7000*1.5));assert.equal(a.owner,FEATURE);assert.equal(a.executable,false);
        assert.ok(scan.text.includes(`"rentEpoch":${MAX}`));assert.equal(a.space,9);
        assert.deepEqual((await rpc(s,"getAccountInfo",[ids[i]])).result.value,a);
    }
    assert.deepEqual((await rpc(s,"getMultipleAccounts",[ids])).result.value,scan.result.map(a=>a.account));
    const contextual=await rpc(s,"getProgramAccounts",[FEATURE,{withContext:true,dataSlice:{offset:0,length:1}}]);
    assert.equal(contextual.result.context.slot,300);assert.equal(contextual.result.value[0].account.data[0],"AQ==");
    const status=(await rpc(s,"svmReplayFork_status")).result;
    assert.deepEqual(status.derivedFeatures,{ids,lamports:String(Math.trunc((128+9)*7000*1.5)),rentSysvar:RENT});
    assert.deepEqual(status.assumedAbsent,[]);
    assert.equal((await rpc(s,"getProgramAccounts",[FEATURE,{filters:null,dataSlice:null}])).result.length,2);
}));
test("feature rent is recomputed for another historical Rent, including fractional truncation",()=>served(boundary(3n,1.25),async s=>{
    assert.equal((await rpc(s,"getAccountInfo",[ids[0]])).result.value.lamports,Math.trunc(137*3*1.25));
}));
test("missing or malformed Rent refuses feature reads even in development mode",async()=>{
    for(const change of [b=>b.accounts.clear(),b=>b.accounts.get(RENT).dataBase64="AA==",b=>b.accounts.get(RENT).owner=FEATURE]){
        const b=boundary();change(b);
        await served(b,async s=>{
            for(const [method,params] of [["getProgramAccounts",[FEATURE]],["getAccountInfo",[ids[0]]],["getMultipleAccounts",[ids]]]){
                const r=await rpc(s,method,params);assert.equal(r.result,undefined);assert.match(r.error.message,/historical Rent/);
            }
            assert.deepEqual((await rpc(s,"svmReplayFork_status")).result.assumedAbsent,[]);
        },true);
    }
});
test("inactive feature ids follow the normal refusal or development rules",async()=>{
    const id=base58Encode(Buffer.alloc(32,23));
    await served(boundary(),async s=>assert.equal((await rpc(s,"getAccountInfo",[id])).error.code,-32010));
    await served(boundary(),async s=>{
        assert.equal((await rpc(s,"getAccountInfo",[id])).result.value,null);
        assert.deepEqual((await rpc(s,"svmReplayFork_status")).result.assumedAbsent,[id]);
    },true);
});
test("an empty binding feature set has an empty scan without assuming absence",()=>{
    const b=boundary();b.features=[];b.accounts.clear();return served(b,async s=>assert.deepEqual((await rpc(s,"getProgramAccounts",[FEATURE])).result,[]));
});
test("feature scans never silently ignore filters",()=>served(boundary(),async s=>{
    assert.equal((await rpc(s,"getProgramAccounts",[FEATURE,{filters:[{dataSize:8}]}])).error.code,-32602);
}));
test("active feature ids are never assumed new by owner scans in development mode",()=>served(boundary(),async s=>{
    assert.equal((await rpc(s,"getTokenAccountsByOwner",[ids[0],{programId:FEATURE}])).error.code,-32601);
    assert.deepEqual((await rpc(s,"svmReplayFork_status")).result.assumedAbsent,[]);
},true));
test("native binding features require unique active ids with valid activation slots",async()=>{
    for(const value of [undefined,[{id:ids[0],activationSlot:301}],[{id:ids[0],activationSlot:-1}],
        [{id:ids[0],activationSlot:1.5}],[{id:ids[0],activationSlot:0},{id:ids[0],activationSlot:0}],
        [{id:"1111",activationSlot:0}]])
        await assert.rejects(nativeFeatures({...boundary(),features:value}),e=>e.code==="BOUNDARY_FORMAT");
    assert.equal((await nativeFeatures({...boundary(),features:[]})).featureImages.size,0);
});
