//! Loopback Solana JSON-RPC server for verified historical state.
import {createServer,type IncomingMessage} from "node:http";
import type {AddressInfo} from "node:net";
import type {AccountImage,Boundary} from "./boundary.js";
const FEATURE_PROGRAM="Feature111111111111111111111111111111111111",RENT_SYSVAR="SysvarRent111111111111111111111111111111111";

export const MAINNET_GENESIS="5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
/** Mainnet epoch schedule (no warmup); SVM Replay reconstructs mainnet only. */
const SLOTS_PER_EPOCH=432000;
const MAX_BODY=1024*1024;

export interface Datasource {
    url:string;
    /** Served images: the exact boundary from SVM Replay's evidence. */
    state:Map<string,AccountImage>;
    /** Keys declared new by the developer (fresh keypairs, own program ids): served as absent. */
    fresh:Set<string>;
    /** Keys requested but outside the boundary; each one was answered with an explicit error. */
    misses:Set<string>;
    /** Development mode only: keys outside the boundary served as absent instead, so new accounts can be created. */
    assumed:Set<string>;
    /** Feature wire images derived from the verified binding and historical Rent, after a feature read. */
    derivedFeatures:Map<string,AccountImage>;
    close():Promise<void>;
}
class RpcFailure extends Error {constructor(readonly code:number,message:string){super(message);}}
const PUBKEY=/^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
/** Built by the runtime for each transaction and never stored, so mainnet RPC also reports it absent. */
const RUNTIME_ONLY=new Set(["Sysvar1nstructions1111111111111111111111111"]);

function account(a:AccountImage,slice?:{offset:number;length:number}):string{
    let data=Buffer.from(a.dataBase64,"base64");const space=data.length;
    if(slice!==undefined)data=data.subarray(slice.offset,slice.offset+slice.length);
    // lamports and rentEpoch are emitted as exact integer literals from their decimal strings.
    return `{"data":[${JSON.stringify(data.toString("base64"))},"base64"],"executable":${a.executable},"lamports":${a.lamports},`+
        `"owner":${JSON.stringify(a.owner)},"rentEpoch":${a.rentEpoch},"space":${space}}`;
}
function sliceOf(config:unknown):{offset:number;length:number}|undefined{
    const slice=(config as {dataSlice?:{offset?:unknown;length?:unknown}}|undefined)?.dataSlice;
    if(slice===undefined||slice===null)return undefined;  // Rust clients send dataSlice: null when unset
    if(!Number.isSafeInteger(slice.offset)||!Number.isSafeInteger(slice.length)||(slice.offset as number)<0||(slice.length as number)<0)
        throw new RpcFailure(-32602,"dataSlice needs non-negative integer offset and length");
    return {offset:slice.offset as number,length:slice.length as number};
}
/** `allowNew` (development only) serves unknown keys as absent and records each in `assumed`; the default fails explicitly. */
export async function serveBoundary(boundary:Boundary,port=0,allowNew=false):Promise<Datasource>{
    const state=new Map(boundary.accounts),fresh=new Set<string>(),misses=new Set<string>(),assumed=new Set<string>();
    const active=new Set(boundary.features.map(f=>f.id)),derivedFeatures=new Map<string,AccountImage>();
    const features=()=>{
        if(derivedFeatures.size!==active.size){
            if(boundary.featureError!==null)throw new RpcFailure(-32012,boundary.featureError.message);
            if(boundary.featureImages===undefined||boundary.featureImages.size!==active.size)throw new RpcFailure(-32012,"native feature account images missing");
            for(const [key,a] of boundary.featureImages)derivedFeatures.set(key,a);
        }
        return derivedFeatures;
    };
    const context=`{"slot":${boundary.targetSlot}}`;
    const lookup=(keys:unknown[],config:unknown):string[]=>{
        const slice=sliceOf(config),outside:string[]=[],changed:string[]=[];
        const values=keys.map(key=>{
            if(typeof key!=="string"||!PUBKEY.test(key))throw new RpcFailure(-32602,"params must be base58 account addresses");
            const found=active.has(key)?features().get(key):state.get(key);if(found!==undefined)return account(found,slice);
            if(boundary.unavailable.has(key)){changed.push(key);return "";}
            if(boundary.absent.has(key)||fresh.has(key)||RUNTIME_ONLY.has(key))return "null";
            if(allowNew){assumed.add(key);return "null";}
            outside.push(key);return "";
        });
        if(changed.length>0){
            for(const key of changed)misses.add(key);
            // Never served from an older image: the engine exposes no bytes for its state at the boundary.
            throw new RpcFailure(-32011,`svm-replay fork: ${changed.join(", ")} changed earlier in the same slot as `+
                `${boundary.signature}; its exact state before that transaction is not exported by this SVM Replay version.`);
        }
        if(outside.length>0){
            for(const key of outside)misses.add(key);
            throw new RpcFailure(-32010,`svm-replay fork: ${outside.length} account(s) are outside the historical state before `+
                `${boundary.signature}: ${outside.join(", ")}. Restart the fork (svm-replay fork start) with --also <address> to read it from `+
                `history, or --new-account <address> for a new keypair or your own program.`);
        }
        return values;
    };
    /** Declared new (or, in development mode, unknown and assumed new): it owned no accounts before the target. */
    const ownsNothing=(key:unknown):boolean=>{
        if(typeof key!=="string"||!PUBKEY.test(key))throw new RpcFailure(-32602,"params must start with a base58 address");
        if(fresh.has(key))return true;
        if(allowNew&&!state.has(key)&&!active.has(key)){assumed.add(key);return true;}
        return false;
    };
    const SCANS=new Set(["getProgramAccounts","getTokenAccountsByOwner","getTokenAccountsByDelegate"]);
    const call=(method:unknown,params:unknown):string=>{
        const args=Array.isArray(params)?params:[];
        if(method==="getProgramAccounts"&&args[0]===FEATURE_PROGRAM){
            const config=args[1] as {filters?:unknown;withContext?:boolean}|undefined;
            if(config?.filters!==undefined&&config.filters!==null&&(!Array.isArray(config.filters)||config.filters.length>0))
                throw new RpcFailure(-32602,"feature scans accept no filters");
            const slice=sliceOf(config),entries=[...features()].map(([key,a])=>`{"pubkey":${JSON.stringify(key)},"account":${account(a,slice)}}`);
            const value=`[${entries.join(",")}]`;
            return config?.withContext===true?`{"context":${context},"value":${value}}`:value;
        }
        // A declared new program or wallet owned no historical accounts. Other scans fail explicitly.
        if(SCANS.has(String(method))&&ownsNothing(args[0])){
            const plain=method==="getProgramAccounts"&&(args[1] as {withContext?:unknown}|null|undefined)?.withContext!==true;
            return plain?"[]":`{"context":${context},"value":[]}`;
        }
        switch(method){
            case "getAccountInfo":return `{"context":${context},"value":${lookup([args[0]],args[1])[0]}}`;
            case "getMultipleAccounts":{
                if(!Array.isArray(args[0]))throw new RpcFailure(-32602,"getMultipleAccounts needs an address list");
                return `{"context":${context},"value":[${lookup(args[0],args[1]).join(",")}]}`;
            }
            case "getEpochInfo":{
                const slotIndex=boundary.targetSlot-boundary.epoch*SLOTS_PER_EPOCH;
                return `{"absoluteSlot":${boundary.targetSlot},"blockHeight":${boundary.blockHeight},"epoch":${boundary.epoch},`+
                    `"slotIndex":${slotIndex},"slotsInEpoch":${SLOTS_PER_EPOCH},"transactionCount":null}`;
            }
            case "getEpochSchedule":
                return `{"firstNormalEpoch":0,"firstNormalSlot":0,"leaderScheduleSlotOffset":${SLOTS_PER_EPOCH},"slotsPerEpoch":${SLOTS_PER_EPOCH},"warmup":false}`;
            case "getGenesisHash":return JSON.stringify(MAINNET_GENESIS);
            case "svmReplayFork_status":return JSON.stringify({signature:boundary.signature,assumedAbsent:[...assumed],misses:[...misses],
                derivedFeatures:{ids:[...derivedFeatures.keys()],lamports:derivedFeatures.size===0?null:derivedFeatures.values().next().value!.lamports,rentSysvar:RENT_SYSVAR}});
            default:throw new RpcFailure(-32601,`svm-replay fork serves historical account reads only; ${String(method)} has no historical source here`);
        }
    };
    const answer=(request:unknown):string=>{
        const {id=null,method,params}=(request??{}) as {id?:unknown;method?:unknown;params?:unknown};
        try {return `{"jsonrpc":"2.0","id":${JSON.stringify(id)},"result":${call(method,params)}}`;}
        catch(error){
            const failure=error instanceof RpcFailure?error:new RpcFailure(-32603,`svm-replay fork internal error: ${String(error).slice(0,300)}`);
            return `{"jsonrpc":"2.0","id":${JSON.stringify(id)},"error":{"code":${failure.code},"message":${JSON.stringify(failure.message)}}}`;
        }
    };
    const body=async(request:IncomingMessage):Promise<string>=>{
        const chunks:Buffer[]=[];let size=0;
        for await(const chunk of request){size+=chunk.length;if(size>MAX_BODY)throw new RpcFailure(-32600,"request body too large");chunks.push(chunk);}
        return Buffer.concat(chunks).toString("utf8");
    };
    const server=createServer((request,response)=>{
        void (async()=>{
            let text:string;
            try {
                if(request.method!=="POST")throw new RpcFailure(-32600,"POST JSON-RPC only");
                const parsed=JSON.parse(await body(request)) as unknown;
                text=Array.isArray(parsed)?`[${parsed.map(answer).join(",")}]`:answer(parsed);
            } catch(error){
                const failure=error instanceof RpcFailure?error:new RpcFailure(-32700,"request is not JSON");
                text=`{"jsonrpc":"2.0","id":null,"error":{"code":${failure.code},"message":${JSON.stringify(failure.message)}}}`;
            }
            response.writeHead(200,{"content-type":"application/json"}).end(text);
        })();
    });
    // Loopback clients may keep idle HTTP connections pooled; leave them open until the server closes.
    server.keepAliveTimeout=0;
    await new Promise<void>((resolve,reject)=>{server.once("error",reject);server.listen(port,"127.0.0.1",()=>resolve());});
    const url=`http://127.0.0.1:${(server.address() as AddressInfo).port}`;
    return {url,state,fresh,misses,assumed,derivedFeatures,
        close:()=>new Promise<void>((resolve,reject)=>server.close(error=>error===undefined?resolve():reject(error)))};
}
