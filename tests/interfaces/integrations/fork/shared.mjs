/** Shared helpers for the opt-in historical fork drivers. Real SVM Replay, real Surfpool; no current-state provider. */
import assert from "node:assert/strict";
import {preparedClient,readResult} from "./offline.mjs";
import {spawn} from "node:child_process";
import {createServer} from "node:net";
import {tmpdir} from "node:os";
import {createHash} from "node:crypto";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {pathToFileURL} from "node:url";
import {footprintTransaction} from "./wire.mjs";

const exactContexts=new WeakMap();
export const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
/** Opens the installed SDK, its fork adapter and a Replay on the installed bundle (data dir and provider limits explicit). */
export async function open(entry,installation,dataDir,alchemyConfig){
    if(process.env.API_ALCHEMY)throw new Error("historical drivers require cached fixtures and no API_ALCHEMY key");
    const sdk=await import(pathToFileURL(resolve(entry)));
    const load=name=>import(pathToFileURL(join(dirname(resolve(entry)),`adapters/fork/${name}.js`)));
    const [fork,boundary,accounts]=await Promise.all([load("index"),load("boundary"),load("accounts")]);
    const pin=JSON.parse(await readFile(join(installation,"installation.json"),"utf8"));
    const replay=await sdk.Replay.open({bundlePath:join(installation,"bundle.json"),bundleSha256:pin.bundleSha256,dataDir,alchemyConfig});
    let serverReplay=replay;
    const sourcePath=process.env.SVM_REPLAY_FORK_SOURCE,sourceSha256=process.env.SVM_REPLAY_FORK_SOURCE_SHA256;
    if(sourcePath!==undefined){
        if(sourceSha256===undefined)throw new Error("SVM_REPLAY_FORK_SOURCE_SHA256 is required for cached-source checks");
        const bytes=await readFile(sourcePath);assert.equal(hash(bytes),sourceSha256,"cached-source manifest pin");
        const manifest=JSON.parse(bytes);
        const offline=await sdk.Replay.open({bundlePath:join(installation,"bundle.json"),bundleSha256:pin.bundleSha256,dataDir,
            source:{path:resolve(sourcePath),sha256:sourceSha256}});
        const context={sdk,fork,offline,original:replay,genesisHash:manifest.identity.genesisHash};
        serverReplay=preparedClient(context);
        exactContexts.set(replay,context);exactContexts.set(serverReplay,context);
    }
    return {sdk,fork,replay:serverReplay,loadClosedBoundary:(client,options,extra=[])=>boundary.loadClosedBoundary(client,options,[...extra,...SUPPORT_ACCOUNTS]),footprintTransaction:(wire,extra)=>footprintTransaction(wire,extra,accounts.base58Decode),base58Encode:accounts.base58Encode,base58Decode:accounts.base58Decode,
        findProgramAddress:accounts.findProgramAddress,programDataAddress:accounts.programDataAddress};
}
export async function output(directory){
    const out=resolve(directory);await mkdir(out,{mode:0o700});
    return (name,value)=>writeFile(join(out,name),JSON.stringify(value,null,1),{flag:"wx",mode:0o600});
}
/**
 * SVM Replay's own answer for `transactionBase64` at the boundary of `signature`: its --replace (and --overrides when
 * given), read from the job's pinned output. With overrides, SVM Replay reports the combined result.
 */
export async function exact(replay,signature,transactionBase64,overrides){
    const context=exactContexts.get(replay);
    if(context===undefined)throw new Error("exact checks require SVM_REPLAY_FORK_SOURCE and SVM_REPLAY_FORK_SOURCE_SHA256 pointing to pinned cached fixtures");
    return readResult(context,{tx:signature,replace:{transactionBase64},...(overrides===undefined?{}:{overrides})});
}
/** State fields SVM Replay records per account transition, computed from a Surfpool account (null = absent). */
export function state(account){
    // Simulation shows a closed account as a zero-lamport account; Solana removes it, as SVM Replay records.
    if(account===null||account===undefined||String(account.lamports)==="0")return {presence:"absent"};
    return {presence:"present",lamports:String(account.lamports),owner:account.owner,executable:account.executable,
        rentEpoch:String(account.rentEpoch),dataHash:hash(Buffer.from(account.data[0],"base64"))};
}
export const FIELDS=["presence","lamports","owner","executable","rentEpoch","dataHash"];
export function sameState(seen,expected,label){
    if(expected.presence==="absent"){assert.equal(seen.presence,"absent",label);return;}
    for(const key of FIELDS)assert.equal(String(seen[key]),String(expected[key]),`${label}: ${key}`);
}
/** Prepared request and result of one SVM Replay job, read from its receipt here, not through the fork's loader. */
export async function evidence(receiptPath){
    const receipt=JSON.parse(await readFile(receiptPath,"utf8")),root=dirname(receiptPath);
    const read=async artifact=>{const bytes=await readFile(join(root,artifact.file));assert.equal(hash(bytes),artifact.sha256);return JSON.parse(bytes);};
    return {prepared:await read(receipt.preparedRequest),result:await read(receipt.output)};
}
/**
 * Expected state of each account just before the target, derived from one job's evidence only: parent-slot images, then
 * every selected predecessor's recorded after-states and every omitted fee, in ledger order. `changed`: written in the slot.
 */
export function expectedBoundary({prepared,result}){
    const states=new Map(),changed=new Set();
    for(const a of prepared.fixture.accounts)states.set(a.pubkey,a.presence==="absent"?{presence:"absent"}:{presence:"present",
        lamports:a.lamports,owner:a.owner,executable:a.executable,rentEpoch:a.rentEpoch,dataHash:hash(Buffer.from(a.dataBase64,"base64"))});
    const events=[...result.prefix.map(tx=>({index:tx.index,tx})),...prepared.fixture.target.omittedFeeEffects.map(fee=>({index:fee.index,fee}))];
    for(const {tx,fee} of events.sort((a,b)=>a.index-b.index)){
        if(tx!==undefined)for(const t of tx.accountTransitions){
            states.set(t.pubkey,t.after);if(JSON.stringify(t.before)!==JSON.stringify(t.after))changed.add(t.pubkey);  // else only loaded
        }
        else {states.set(fee.feePayer,{...states.get(fee.feePayer),lamports:fee.postLamports});changed.add(fee.feePayer);}
    }
    return {states,changed};
}
export async function accounts(surfpool,keys){
    const out=[];
    for(let i=0;i<keys.length;i+=100){
        const {value}=await surfpool.call("getMultipleAccounts",[keys.slice(i,i+100),{encoding:"base64",commitment:"processed"}]);
        out.push(...value);
    }
    return out;
}
/** Simulation of `transactionBase64` on Surfpool with the post-state of `keys`. */
export async function simulate(surfpool,transactionBase64,keys=[]){
    const {value}=await surfpool.call("simulateTransaction",[transactionBase64,{encoding:"base64",sigVerify:false,replaceRecentBlockhash:false,
        accounts:keys.length===0?undefined:{encoding:"base64",addresses:keys}}]);
    return value;
}
/** G0: status and exact error; G1: plus logs; G2: plus compute units. Returns the highest grade reached. */
export function grade(simulation,expected){
    const ok=simulation.err===null,expectedOk=expected.status==="ok";
    if(ok!==expectedOk)return "none";
    // A failure must be the same failure: SVM Replay records the RPC error shape as a JSON string.
    if(!ok&&JSON.stringify(simulation.err)!==expected.normalizedError)return "none";
    if(JSON.stringify(simulation.logs)!==JSON.stringify(expected.logs))return "G0";
    return simulation.unitsConsumed===expected.computeUnits?"G2":"G1";
}
/**
 * Every written account after a successful simulation equals the expected after-state. One difference is allowed, and
 * proven: Surfpool's blockhash is synthetic, so an advanced durable nonce differs in exactly its 32-byte nonce field, and
 * the account rebuilt with the historical blockhash there hashes to the verified after-state.
 */
export function effects(simulation,transitions,previousBlockhash,base58Decode){
    if(simulation.err!==null)return {failed:simulation.err};  // Surfpool returns no account states for a failed simulation
    assert.ok(Array.isArray(simulation.accounts)&&simulation.accounts.length===transitions.length,"a successful simulation returns every written account");
    const nonceField=[];
    transitions.forEach((t,i)=>{
        const a=simulation.accounts[i],seen=state(a);
        if(FIELDS.every(f=>String(seen[f])===String(t.after[f]))||(t.after.presence==="absent"&&seen.presence==="absent"))return;
        const bytes=Buffer.from(a?.data[0]??"","base64");
        assert.ok(a?.owner==="11111111111111111111111111111111"&&bytes.length===80&&FIELDS.slice(0,5).every(f=>String(seen[f])===String(t.after[f])),
            `${t.pubkey} differs from SVM Replay's verified after-state`);
        createHash("sha256").update("DURABLE_NONCE").update(base58Decode(previousBlockhash)).digest().copy(bytes,40);
        assert.equal(hash(bytes),t.after.dataHash,`${t.pubkey}: only the durable nonce field may differ`);nonceField.push(t.pubkey);
    });
    return {matched:transitions.length-nonceField.length,nonceFieldOnly:nonceField};
}
/** Static keys and instruction data offsets of a serialized transaction. */
export function parse(transactionBase64,base58Encode){
    const tx=Buffer.from(transactionBase64,"base64");let at=0;
    const compact=()=>{let v=0;for(let shift=0;;shift+=7){const b=tx[at++];v|=(b&0x7f)<<shift;if((b&0x80)===0)return v;}};
    const signatures=compact();at+=64*signatures;if((tx[at]&0x80)!==0)at+=1;at+=3;
    const keys=[];for(let n=compact();n>0;n--){keys.push(base58Encode(tx.subarray(at,at+32)));at+=32;}
    at+=32;const instructions=[];
    for(let n=compact();n>0;n--){const program=keys[tx[at++]];const count=compact();at+=count;const length=compact();instructions.push({program,at,length});at+=length;}
    return {tx,keys,instructions};
}
const COMPUTE_BUDGET="ComputeBudget111111111111111111111111111111";
/** The transaction with its SetComputeUnitLimit lowered to `units`, or undefined when it sets no limit. */
export function lowerComputeLimit(transactionBase64,units,base58Encode){
    const {tx,instructions}=parse(transactionBase64,base58Encode);
    const limit=instructions.find(i=>i.program===COMPUTE_BUDGET&&i.length===5&&tx[i.at]===2);
    if(limit===undefined)return undefined;
    const out=Buffer.from(tx);out.writeUInt32LE(units,limit.at+1);return out.toString("base64");
}
/** The transaction with the last data byte of its last non-compute-budget instruction flipped. */
export function flipLastData(transactionBase64,base58Encode){
    const {tx,instructions}=parse(transactionBase64,base58Encode);
    const last=instructions.filter(i=>i.program!==COMPUTE_BUDGET&&i.length>0).at(-1);
    if(last===undefined)return undefined;
    const out=Buffer.from(tx);out[last.at+last.length-1]^=1;return out.toString("base64");
}
/** True when nothing listens on 127.0.0.1:port any more. */
export async function closed(port){
    const {connect}=await import("node:net");
    return new Promise((done,fail)=>{const socket=connect({host:"127.0.0.1",port:Number(port)});socket.setTimeout(2000);
        socket.once("connect",()=>{socket.destroy();done(false);});
        socket.once("error",e=>e.code==="ECONNREFUSED"?done(true):fail(e));
        socket.once("timeout",()=>{socket.destroy();fail(new Error(`port ${port} probe timed out`));});});
}

/** Driver-owned support reads; the historical server has no consumer-specific account list. */
export const SUPPORT_ACCOUNTS=["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL","Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
    "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr","So11111111111111111111111111111111111111112"];
export async function freePort(){
    return new Promise((resolve,reject)=>{const s=createServer();s.once("error",reject);
        s.listen(0,"127.0.0.1",()=>{const {port}=s.address();s.close(e=>e?reject(e):resolve(port));});});
}
export async function rpc(url,method,params=[],timeoutMs=120000){
    const response=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},signal:AbortSignal.timeout(timeoutMs),
        body:JSON.stringify({jsonrpc:"2.0",id:1,method,params},(_k,v)=>typeof v==="bigint"?`\u0000u64:${v}`:v).replace(/"\\u0000u64:(\d+)"/g,"$1")});
    const body=JSON.parse(await response.text(),(_k,v,c)=>typeof v==="number"&&!Number.isSafeInteger(v)?c.source:v);
    if(body.error!==undefined)throw Object.assign(new Error(`${method}: ${body.error.message}`),{code:"SURFPOOL_RPC",details:{code:body.error.code}});return body.result;
}
/** A user's own Surfpool command, run only by the driver, with bounded startup and cleanup. */
export async function startSurfpool({binary,datasource,matchDatasource=false,blockMode="manual",extra=[]}){
    for(let attempt=0;attempt<3;attempt++){
        const port=await freePort(),wsPort=await freePort(),url=`http://127.0.0.1:${port}`;
        const args=["start","--rpc-url",datasource,...(matchDatasource?["--match-datasource"]:[]),"--no-deploy","--yes","--ci","--no-studio",
            "--port",String(port),"--ws-port",String(wsPort),"--block-production-mode",blockMode,
            "--skip-signature-verification","--skip-blockhash-check","--airdrop-amount","0","--log-bytes-limit","0",
            "--surfnet-id",`fork-driver-${process.pid}-${port}`,...extra];
        const child=spawn(binary,args,{cwd:tmpdir(),stdio:["ignore","ignore","pipe"]});
        let stderr="",exited=false,startError;
        child.stderr.on("data",b=>{stderr=(stderr+b.toString()).slice(-16384);});
        const done=new Promise(resolve=>{child.once("error",e=>{startError=e;exited=true;resolve();});child.once("exit",()=>{exited=true;resolve();});});
        const stop=async()=>{if(exited)return;child.kill("SIGTERM");let timer;
            await Promise.race([done,new Promise(r=>{timer=setTimeout(r,5000);})]);clearTimeout(timer);
            if(!exited){child.kill("SIGKILL");await done;}};
        try {
            for(const deadline=Date.now()+60000;;){
                if(startError)throw startError;
                if(exited)throw new Error(`Surfpool exited during startup: ${stderr}`);
                try {if(await rpc(url,"getHealth",[],2000)==="ok")return {url,args,call:(m,p=[],t=120000)=>rpc(url,m,p,t),stop};}
                catch(error){if(Date.now()>deadline)throw new Error(`Surfpool startup health at ${url}: ${error.message}; ${stderr}`);}
                if(Date.now()>deadline)throw new Error(`Surfpool not healthy at ${url}: ${stderr}`);
                await new Promise(r=>setTimeout(r,250));
            }
        } catch(error){await stop();if(attempt<2&&/address already in use|EADDRINUSE/i.test(stderr)){console.error(`retry Surfpool port collision: ${stderr}`);continue;}throw error;}
    }
    throw new Error("Surfpool port retries exhausted");
}
/** Consumer requests of legacy programs' loader-v3 addresses are explicit driver declarations, never server inference. */
export function declareProgramData(built,addressOf){
    const keys=SUPPORT_ACCOUNTS.filter(key=>{const a=built.datasource.state.get(key);
        return a!==undefined&&a.executable&&a.owner!=="BPFLoaderUpgradeab1e11111111111111111111111";}).map(addressOf);
    for(const key of keys)built.datasource.fresh.add(key);
    return keys;
}

export {anchorConfig,anchorPorts,prepareProgramKeys,buildAnchor} from "./anchor-tools.mjs";
