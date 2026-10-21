import {test} from "node:test";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {base58Encode,base58Decode,programDataAddress} from "../dist/adapters/fork/accounts.js";
import {loadBoundary,loadClosedBoundary} from "../dist/adapters/fork/boundary.js";
import {projectBoundary,loadedKeys} from "./native-boundary.mjs";

// Offline: a scripted Replay whose jobs write real receipt artifacts, so the loaders run unchanged.
const sha=bytes=>createHash("sha256").update(bytes).digest("hex");
const key=n=>base58Encode(Buffer.alloc(32,n));
const TOKEN="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",UPGRADEABLE="BPFLoaderUpgradeab1e11111111111111111111111";
const CLOCK="SysvarC1ock11111111111111111111111111111111",SYSVAR="Sysvar1111111111111111111111111111111111111";
const image=(pubkey,{owner="11111111111111111111111111111111",data=[],lamports="10",executable=false}={})=>
    ({pubkey,presence:"present",lamports,owner,executable,rentEpoch:"18446744073709551615",dataBase64:Buffer.from(data).toString("base64")});
const tokenAccount=(pubkey,mint)=>image(pubkey,{owner:TOKEN,data:Buffer.concat([base58Decode32(mint),Buffer.alloc(133)])});
function base58Decode32(k){for(let n=0;n<256;n++)if(key(n)===k)return Buffer.alloc(32,n);throw new Error(`test key ${k}`);}
const clockData=Buffer.alloc(40);clockData.writeBigUInt64LE(422490991n,0);clockData.writeBigUInt64LE(978n,16);
const clock={...image(CLOCK,{owner:SYSVAR,data:clockData}),presence:undefined};
const before=a=>({presence:"present",lamports:a.lamports,owner:a.owner,dataHash:sha(Buffer.from(a.dataBase64,"base64"))});
const P=key(1),X=key(2),Q=key(3),TABLE=key(4),A1=key(5),A2=key(6);

/** Versioned transaction: static keys, then one lookup table read with the given (writable, readonly) indexes. */
function transaction(keys,table){
    const lookups=table===undefined?[Buffer.from([0])]:[Buffer.from([1]),base58Decode32(table),Buffer.from([1,0,1,1])];
    return Buffer.concat([Buffer.from([1]),Buffer.alloc(64),Buffer.from([0x80,1,0,0,keys.length]),...keys.map(base58Decode32),
        Buffer.alloc(32,9),Buffer.from([0]),...lookups]).toString("base64");
}
/** Each job: {accounts, prefixTouched, omittedFeePayers, original, journal}. `respond` builds one from the request. */
async function scripted(respond){
    const dir=await mkdtemp(join(tmpdir(),"fork-boundary-")),requests=[];let originalWire;
    const replay={projectBoundary,async replay(options){
        requests.push(options);const job=respond(options,requests.length);const n=requests.length;
        if(n===1)originalWire=job.transactionBase64;
        const transactionBase64=job.transactionBase64??originalWire;
        const prepared={family:"v4-2",candidate:{activeFeatures:["f1"]},rawBlockBase64:Buffer.from(JSON.stringify({result:{blockHeight:400539222}})).toString("base64"),
            fixture:{accounts:job.accounts,clock,runtime:{binding:{features:[{id:key(20),activationSlot:123}]},genericSysvars:[{pubkey:CLOCK,dataSha256:sha(clockData)}]},
                target:{signature:"sig",transactionBase64,replacementTransactionBase64:options.replace?.transactionBase64??null,parentSlot:422490990,targetSlot:422490991,prefixSignatures:job.prefixTouched.map(()=>"p"),
                    omittedFeeEffects:(job.omittedFeePayers??[]).map((feePayer,index)=>({index,signature:"s",feePayer,feeLamports:"5000"}))}}};
        // Predecessors list what they changed and what they only loaded (`prefixLoaded`, before equals after).
        const changed=pubkey=>({pubkey,before:{presence:"present",lamports:"1"},after:{presence:"present",lamports:"2"}});
        const loaded=pubkey=>({pubkey,before:{presence:"present",lamports:"1"},after:{presence:"present",lamports:"1"}});
        const result={prefix:job.prefixTouched.map((keys,i)=>({accountTransitions:[...keys.map(changed),...(i===0?job.prefixLoaded??[]:[]).map(loaded)]})),
            original:{accountTransitions:job.original??[]}};
        const files={};
        // The engine writes u64 values as plain JSON numbers: rent_epoch u64::MAX must survive parsing exactly.
        const journal=JSON.stringify(job.journal??begin([[P,image(P)]])).replace(/"rent_epoch":"(\d+)"/g,'"rent_epoch":$1');
        for(const [name,text] of [["prepared",JSON.stringify(prepared)],["output",JSON.stringify(result)],["journal",journal]]){
            const bytes=Buffer.from(text);files[name]={file:`${name}-${n}.json`,sha256:sha(bytes)};
            await writeFile(join(dir,files[name].file),bytes);
        }
        const receipt={outcome:"COMPLETED",receiptPath:join(dir,`receipt-${n}.json`),preparedRequest:files.prepared,output:files.output,
            controlVerification:{status:"PASS"},verification:{status:"PASS"},trace:{exports:[{...files.journal,phase:"original-control"}]},
            ...(job.runtime===undefined?{}:{runtime:job.runtime})};
        await writeFile(receipt.receiptPath,JSON.stringify(receipt));
        return {receipt,response:{trace:{exports:[files.journal]}}};
    }};
    return {replay,requests,dir,cleanup:()=>rm(dir,{recursive:true,force:true})};
}
const begin=entries=>[{event:{kind:"transaction_begin",accounts:entries.map(([pubkey,a])=>({pubkey,state:a===null?null:
    {lamports:Number(a.lamports),owner:a.owner,executable:a.executable,rent_epoch:a.rentEpoch,data:[...Buffer.from(a.dataBase64,"base64")]}}))}}];

test("static accounts come from the journal snapshot; lookup entries from untouched parent images; incidental keys are not served",async()=>{
    const parentP=image(P,{lamports:"100"}),afterPrefix=image(P,{lamports:"95"}),table=image(TABLE,{owner:"AddressLookupTab1e1111111111111111111111111",
        data:Buffer.concat([Buffer.alloc(56),base58Decode32(A1),base58Decode32(A2)])});
    const {replay,cleanup}=await scripted(()=>({transactionBase64:transaction([P,X],TABLE),
        accounts:[parentP,image(X),table,image(A1),image(A2),image(Q)],prefixTouched:[[P,A2,Q]],
        journal:begin([[P,afterPrefix],[X,image(X)],[Q,image(Q,{lamports:"7"})]]),original:[{pubkey:P,before:before(afterPrefix)}]}));
    try {
        const b=await loadBoundary(replay,{tx:"sig"});
        assert.equal(b.accounts.get(P).lamports,"95");  // after the same-slot predecessor, not the parent-slot image
        assert.ok(b.accounts.has(X)&&b.accounts.has(A1)&&b.accounts.has(CLOCK));
        assert.equal(b.accounts.get(TABLE).dataBase64,table.dataBase64,"the lookup table itself: Surfpool resolves the message with it");
        assert.ok(!b.accounts.has(Q),"a predecessor's own account is not part of the target boundary");
        assert.deepEqual([...b.unavailable],[A2]);assert.equal(b.epoch,978);assert.equal(b.blockHeight,400539222);
        assert.equal(b.predecessors,1);assert.equal(b.evidence.length,1);
        assert.deepEqual(b.features,[{id:key(20),activationSlot:123}],"activation slots come from the verified binding, not candidate feature ids");
        assert.deepEqual(b.activeFeatures,[key(20)],"the public id list also comes from the verified binding");
        assert.equal(b.accounts.get(X).rentEpoch,"18446744073709551615");
    } finally {await cleanup();}
});
const enter=entries=>({event:{kind:"enter",call_id:1,parent_id:null,invocation:{accounts:begin(entries)[0].event.accounts}}});
test("lookup-table accounts changed earlier come from their first use; unchanged ones must equal their parent image",async()=>{
    const table=image(TABLE,{owner:"AddressLookupTab1e1111111111111111111111111",data:Buffer.concat([Buffer.alloc(56),base58Decode32(A1),base58Decode32(A2)])});
    const job=entries=>()=>({transactionBase64:transaction([P,X],TABLE),accounts:[image(P),image(X),table,image(A1),image(A2)],prefixTouched:[[A2]],
        journal:[...begin([[P,image(P)],[X,image(X)]]),enter(entries)]});
    const used=await scripted(job([[A1,image(A1)],[A2,image(A2,{lamports:"33"})]]));
    try {
        const b=await loadBoundary(used.replay,{tx:"sig"});
        assert.equal(b.accounts.get(A2).lamports,"33");assert.deepEqual([...b.unavailable],[]);
    } finally {await used.cleanup();}
    // A missing account reaches an instruction as an empty 0-lamport account: it is absent, consistent with the parent.
    const missing=await scripted(()=>({transactionBase64:transaction([P,X],TABLE),accounts:[image(P),image(X),table,image(A1),{pubkey:A2,presence:"absent"}],
        prefixTouched:[],journal:[...begin([[P,image(P)],[X,image(X)]]),enter([[A2,image(A2,{lamports:"0"})]])]}));
    try {const b=await loadBoundary(missing.replay,{tx:"sig"});assert.ok(b.absent.has(A2)&&!b.accounts.has(A2));} finally {await missing.cleanup();}
    const inconsistent=await scripted(job([[A1,image(A1,{lamports:"99"})]]));
    try {await assert.rejects(loadBoundary(inconsistent.replay,{tx:"sig"}),e=>e.code==="BOUNDARY_MISMATCH");} finally {await inconsistent.cleanup();}
});
test("a slot-start sysvar comes from its first use, never from its parent-slot image",async()=>{
    const HASHES="SysvarRecentB1ockHashes11111111111111111111",parentHashes=image(HASHES,{owner:SYSVAR,data:[1]});
    const table=image(TABLE,{owner:"AddressLookupTab1e1111111111111111111111111",
        data:Buffer.concat([Buffer.alloc(56),Buffer.from(base58Decode(HASHES)),base58Decode32(A1)])});
    const run=async(journal,check)=>{
        const s=await scripted(()=>({transactionBase64:transaction([P,X],TABLE),accounts:[image(P),image(X),table,parentHashes,image(A1)],prefixTouched:[],journal}));
        try {await check(await loadBoundary(s.replay,{tx:"sig"}));} finally {await s.cleanup();}
    };
    await run([...begin([[P,image(P)],[X,image(X)]]),enter([[HASHES,image(HASHES,{owner:SYSVAR,data:[2]})]])],
        b=>assert.equal(b.accounts.get(HASHES).dataBase64,Buffer.from([2]).toString("base64")));
    await run(begin([[P,image(P)],[X,image(X)]]),b=>{assert.ok(b.unavailable.has(HASHES));assert.ok(!b.accounts.has(HASHES));});
});
test("the runtime is named by the engine's receipt; a 0.1.0 engine names none",async()=>{
    const runtime={family:"v4-2",name:"Agave 4.2.1",agave:"4.2.1",litesvm:"0.16.0",reviewedSlots:[[438499848,451646918]]};
    for(const named of [runtime,undefined]){
        const s=await scripted(()=>({transactionBase64:transaction([P,X]),accounts:[image(P),image(X)],prefixTouched:[],
            journal:begin([[P,image(P)],[X,image(X)]]),runtime:named}));
        try {assert.deepEqual((await loadBoundary(s.replay,{tx:"sig"})).runtime,named??null);} finally {await s.cleanup();}
    }
});
test("a fork that starts again reuses its verified jobs; changed evidence, or noCache, runs them again",async()=>{
    const s=await scripted(()=>({transactionBase64:transaction([P,X]),accounts:[image(P),image(X)],prefixTouched:[],journal:begin([[P,image(P)],[X,image(X)]])}));
    const data=await mkdtemp(join(tmpdir(),"fork-jobs-"));
    s.replay.forkJobScope=()=>({bundleSha256:"b".repeat(64),dataDir:data});
    try {
        const first=await loadBoundary(s.replay,{tx:"sig"}),again=await loadBoundary(s.replay,{tx:"sig"});
        assert.equal(s.requests.length,1,"the second start replays nothing");assert.deepEqual(again.accounts,first.accounts);
        await writeFile(join(s.dir,"output-1.json"),"{}");  // the kept job's evidence no longer matches its pin
        await loadBoundary(s.replay,{tx:"sig"});assert.equal(s.requests.length,2);
        await loadBoundary(s.replay,{tx:"sig",noCache:true});assert.equal(s.requests.length,3);
    } finally {await s.cleanup();await rm(data,{recursive:true,force:true});}
});
test("an account that did not exist yet is refused with the way out: declare it new",async()=>{
    const replay={async replay(){return {receipt:{outcome:"UNSUPPORTED",error:{code:"UNSUPPORTED_HISTORICAL_ACCOUNT",
        message:"X: archive null has no independent historical absence evidence",details:{reason:"unproven-account-absence",pubkey:"X"}}}};}};
    await assert.rejects(loadBoundary(replay,{tx:"sig"}),
        error=>error.code==="BOUNDARY_UNVERIFIED"&&/UNSUPPORTED \(X: archive null.*\)\. X has no account .*--new-account/.test(error.message));
});
test("a snapshot that differs from the verified before-state is rejected",async()=>{
    const {replay,cleanup}=await scripted(()=>({transactionBase64:transaction([P,X]),accounts:[image(P),image(X)],prefixTouched:[],
        journal:begin([[P,image(P,{lamports:"11"})],[X,image(X)]]),original:[{pubkey:P,before:before(image(P))}]}));
    try {await assert.rejects(loadBoundary(replay,{tx:"sig"}),e=>e.code==="BOUNDARY_MISMATCH");} finally {await cleanup();}
});
test("widening proves extras and companions in rounds: present, absent, written earlier (snapshot or unavailable)",async()=>{
    const Y=key(10),M=key(11),Z=key(12),W=key(13),PROGRAM=key(14),PD=key(15),V=key(16);
    const program=image(PROGRAM,{owner:UPGRADEABLE,executable:true,data:Buffer.concat([Buffer.from([2,0,0,0]),base58Decode32(PD)])});
    const legacy=image(TOKEN,{owner:"BPFLoader2111111111111111111111111111111111",executable:true});
    const known=new Map([[Y,tokenAccount(Y,M)],[M,image(M,{owner:TOKEN})],[W,image(W)],[V,image(V)],[PD,image(PD,{owner:UPGRADEABLE})],[TOKEN,legacy]]);
    const {replay,requests,cleanup}=await scripted((options,n)=>{
        if(n===1)return {transactionBase64:transaction([P,PROGRAM]),accounts:[image(P),program],prefixTouched:[],journal:begin([[P,image(P)],[PROGRAM,program]])};
        const keys=loadedKeys(options.replace.transactionBase64,new Map()).static.slice(1);
        // W and V were written earlier in the slot; only W is a static key of its writer, so only W is in the snapshot.
        return {accounts:keys.map(k=>known.has(k)?known.get(k):{pubkey:k,presence:"absent"}),prefixTouched:[[W,V]],prefixLoaded:[Y],
            journal:begin([[P,image(P)],[W,image(W,{lamports:"77"})]])};
    });
    try {
        const b=await loadClosedBoundary(replay,{tx:"sig"},[Y,Z,W,V]);
        assert.equal(requests.length,3);assert.equal(b.evidence.length,3);
        assert.ok(requests.slice(1).every(r=>r.collect?.calls===true),"every widening job collects the engine snapshot");
        assert.ok(b.accounts.has(Y)&&b.accounts.has(M)&&b.accounts.has(PD),"extra, its mint and the program's ProgramData");
        assert.equal(b.accounts.get(Y).lamports,"10");  // only loaded earlier in the slot: its parent-slot image
        assert.equal(b.accounts.get(W).lamports,"77");  // the state after its earlier writer, not the parent image
        assert.ok(b.absent.has(Z));assert.deepEqual([...b.unavailable],[V]);
        // A non-upgradeable program's derived ProgramData address is not requested (execution never reads it).
        assert.ok(!requests.slice(1).some(r=>loadedKeys(r.replace.transactionBase64,new Map()).static.includes(programDataAddress(TOKEN))));
        // Round 2 asks only for what round 1 revealed: the mint.
        assert.deepEqual(loadedKeys(requests[2].replace.transactionBase64,new Map()).static.slice(1),[M]);
    } finally {await cleanup();}
});
test("an account the engine did not prove, and endless companion growth, fail explicitly",async()=>{
    const first=()=>({transactionBase64:transaction([P]),accounts:[image(P)],prefixTouched:[],journal:begin([[P,image(P)]])});
    const silent=await scripted((options,n)=>n===1?first():{accounts:[],prefixTouched:[]});
    try {await assert.rejects(loadClosedBoundary(silent.replay,{tx:"sig"},[key(20)]),e=>e.code==="BOUNDARY_INCOMPLETE");} finally {await silent.cleanup();}
    let next=30;  // every proven token account names a new mint, which is another token account
    const growing=await scripted((options,n)=>n===1?first():{prefixTouched:[],
        accounts:loadedKeys(options.replace.transactionBase64,new Map()).static.slice(1).map(k=>[TOKEN].includes(k)?image(k):tokenAccount(k,key(next++)))});
    try {await assert.rejects(loadClosedBoundary(growing.replay,{tx:"sig"},[key(29)]),e=>e.code==="BOUNDARY_INCOMPLETE"&&/4 rounds/.test(e.message));}
    finally {await growing.cleanup();}
});
