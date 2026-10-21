// Test transport into the real native boundary projector; no mock projection algorithm.
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {fileURLToPath} from "node:url";
import {run} from "../dist/process.js";
import {ReplayError} from "../dist/types.js";
import {base58Encode} from "../dist/adapters/fork/accounts.js";
import {createHash} from "node:crypto";
export async function projectBoundary(input){
    const directory=await mkdtemp(join(tmpdir(),"native-boundary-"));
    try {
        const path=join(directory,"request.json");await writeFile(path,JSON.stringify(input));
        const binary=process.env.SVM_REPLAY_BOUNDARY_BINARY??fileURLToPath(new URL("../../../../target/debug/svm-replay",import.meta.url));
        const result=await run(binary,["boundary","--request",path],{timeoutMs:30000,maxBytes:268435456,alchemy:false});
        const value=JSON.parse(result.stdout);
        if(result.exitCode!==0)throw new ReplayError(value.error.code,value.error.message);
        return value;
    }finally{await rm(directory,{recursive:true,force:false});}
}
// Independent wire reader for test assertions, not a shipping historical-state implementation.
export function loadedKeys(base64,tables=new Map()){
    const tx=Buffer.from(base64,"base64");let at=0;
    const compact=()=>{let value=0;for(let s=0;;s+=7){const b=tx[at++];if(b===undefined||s>14)throw new Error("truncated test transaction");value|=(b&127)<<s;if(!(b&128))return value;}};
    const take=n=>{const b=tx.subarray(at,at+n);at+=n;return b;};
    take(64*compact());const versioned=(tx[at]&128)!==0;take(versioned?4:3);
    const keys=Array.from({length:compact()},()=>base58Encode(take(32)));take(32);
    for(let n=compact();n>0;n--){take(1);take(compact());take(compact());}
    const lookup=[],used=[];
    if(versioned)for(let n=compact();n>0;n--){
        const table=base58Encode(take(32)),indices=[...take(compact()),...take(compact())];used.push(table);
        const data=Buffer.from(tables.get(table).dataBase64,"base64");
        for(const index of indices)lookup.push(base58Encode(data.subarray(56+32*index,88+32*index)));
    }
    return {static:keys,tables:used,lookup};
}
export async function nativeFeatures(b){
    const directory=await mkdtemp(join(tmpdir(),"feature-jobs-"));
    try {
        const key=base58Encode(Buffer.alloc(32,7)),data=Buffer.alloc(40);data.writeBigUInt64LE(BigInt(b.targetSlot),0);
        const sha=raw=>createHash("sha256").update(raw).digest("hex");
        const clock={pubkey:"SysvarC1ock11111111111111111111111111111111",owner:"Sysvar1111111111111111111111111111111111111",executable:false,
            lamports:"1",rentEpoch:"18446744073709551615",dataBase64:data.toString("base64")};
        const wire=Buffer.concat([Buffer.from([1]),Buffer.alloc(64),Buffer.from([1,0,0,1]),Buffer.alloc(32,7),Buffer.alloc(32,9),Buffer.from([0])]).toString("base64");
        const prepared={family:"v4-2",rawBlockBase64:Buffer.from(JSON.stringify({result:{blockHeight:1}})).toString("base64"),
            fixture:{accounts:[...b.accounts.values()].map(a=>({...a,presence:"present"})),clock,runtime:{binding:{features:b.features},genericSysvars:[{pubkey:clock.pubkey,dataSha256:sha(data)}]},
                target:{signature:"sig",transactionBase64:wire,parentSlot:b.targetSlot-1,targetSlot:b.targetSlot,prefixSignatures:[]}}};
        const result={prefix:[],original:{accountTransitions:[]}};
        const journal=[{event:{kind:"transaction_begin",accounts:[{pubkey:key,state:null}]}}];
        const pins={};
        for(const [name,value] of Object.entries({prepared,result,journal})){
            const raw=JSON.stringify(value);await writeFile(join(directory,`${name}.json`),raw);pins[name]={file:`${name}.json`,sha256:sha(raw)};
        }
        const path=join(directory,"receipt.json"),raw=JSON.stringify({outcome:"COMPLETED",preparedRequest:pins.prepared,output:pins.result,
            controlVerification:{status:"PASS"},verification:{status:"PASS"},trace:{exports:[{...pins.journal,phase:"original-control"}]}});
        await writeFile(path,raw);
        const output=await projectBoundary({schema:"svm-replay-boundary-request/v1",jobs:[{receiptPath:path,receiptSha256:sha(raw),trace:pins.journal}],extra:[]});
        return {featureImages:new Map(Object.entries(output.boundary.featureImages)),featureError:output.boundary.featureError};
    }finally{await rm(directory,{recursive:true,force:false});}
}
