import {spawn} from "node:child_process";
import {readFile,stat} from "node:fs/promises";
import {basename,dirname} from "node:path";
import {verifyArtifact} from "./io.js";
import {decodeJson,encodeJson} from "./request.js";
import {sessionArguments,type ForkAtOptions} from "./session-options.js";
import {ReplayError,type Json,type JsonObject,type StateOverride} from "./types.js";

export interface HistoricalSession {
    /** HTTP JSON-RPC; WebSocket signature subscriptions are on `wsUrl` (port + 1). */
    readonly url:string; readonly wsUrl:string|undefined; readonly pins:JsonObject;
    snapshot():Promise<number>; select(forkId:number):Promise<JsonObject>;
    reset():Promise<JsonObject>; closeFork(forkId:number):Promise<void>; close():Promise<void>;
    /** Explicit hypothetical wallet funding; does not change pinned historical evidence. */
    setBalance(pubkey:string,lamports:bigint):Promise<JsonObject>;
    /** Explicit native account edits. `program: "@path/to/program.so"` installs that ELF, as the CLI does. */
    overrideAccounts(entries:StateOverride[]):Promise<JsonObject>;
    /** Explicit local slots in the boundary's epoch: loader deployments become visible; time moves only when given. */
    advance(slots?:number,options?:{unixTimestamp?:number|bigint}):Promise<JsonObject>;
}

const PROGRAM_OWNER="BPFLoader2111111111111111111111111111111111",MAX_PROGRAM=10*1024*1024;

/** File reading only: `program` becomes the same owner/executable/data the CLI writes. */
export async function resolveProgram(entry:StateOverride):Promise<StateOverride> {
    if(entry.program===undefined)return entry;
    const {program,...rest}=entry;
    if(!program.startsWith("@")||program.length===1)throw new ReplayError("INVALID_STATE_OVERRIDE",'program is "@path/to/program.so"');
    if(rest.owner!==undefined||rest.executable!==undefined||rest.dataBase64!==undefined)throw new ReplayError("INVALID_STATE_OVERRIDE","program sets owner, executable and dataBase64 itself");
    const path=program.slice(1);
    if((await stat(path)).size>MAX_PROGRAM)throw new ReplayError("INPUT_IO",`${path}: program exceeds 10 MiB`);
    const bytes=await readFile(path);
    if(!bytes.subarray(0,4).equals(Buffer.from([0x7f,0x45,0x4c,0x46])))throw new ReplayError("INVALID_STATE_OVERRIDE",`${path} is not a program (.so ELF file)`);
    return {...rest,owner:PROGRAM_OWNER,executable:true,dataBase64:bytes.toString("base64")};
}

function object(value:Json):JsonObject {
    if(value===null||Array.isArray(value)||typeof value!=="object")throw new ReplayError("SESSION_PROTOCOL","native session object required");
    return value;
}

/** The native CLI owns its historical workers; this client only owns that CLI. */
export async function openSession(binary:string,binarySha256:string,bundleArgs:string[],dataDir:string|undefined,options:ForkAtOptions):Promise<HistoricalSession> {
    if(process.platform!=="linux")throw new ReplayError("UNSUPPORTED_PLATFORM","historical SDK parent ownership is currently qualified on Linux only");
    await verifyArtifact(dirname(binary),{file:basename(binary),sha256:binarySha256},512*1024*1024);
    if(options.signal?.aborted)throw new ReplayError("CANCELLED","cancelled before opening historical fork");
    const command=sessionArguments(options,bundleArgs,dataDir);
    const child=spawn(binary,command.args,{shell:false,stdio:["ignore","pipe","pipe"],env:command.env});
    let terminal=false,buffer="",stderr="",bytes=0,failure:ReplayError|undefined,readySeen=false,closedSeen=false;
    let kill:ReturnType<typeof setTimeout>|undefined;
    let readyResolve:(value:JsonObject)=>void,readyReject:(error:unknown)=>void;
    const readyPromise=new Promise<JsonObject>((resolve,reject)=>{readyResolve=resolve;readyReject=reject;});
    const fail=(error:ReplayError)=>{
        if(failure===undefined)failure=error;
        readyReject(error);
        if(!terminal){child.kill("SIGTERM");if(kill===undefined)kill=setTimeout(()=>child.kill("SIGKILL"),10000);}
    };
    const abort=()=>fail(new ReplayError("CANCELLED","historical fork cancelled"));
    options.signal?.addEventListener("abort",abort,{once:true});
    if(options.signal?.aborted)abort();
    const timer=setTimeout(()=>fail(new ReplayError("TIMEOUT","historical fork lifetime exceeded")),command.timeoutMs);
    child.stdout.on("data",(data:Buffer)=>{
        bytes+=data.length;if(bytes>2*1024*1024){fail(new ReplayError("OUTPUT_LIMIT","historical fork diagnostics exceed 2MiB"));return;}
        buffer+=data.toString("utf8");
        let end:number;
        while((end=buffer.indexOf("\n"))!==-1){
            const line=buffer.slice(0,end);buffer=buffer.slice(end+1);
            try {
                const value=object(decodeJson(line));
                if(!readySeen){
                    if(value.schema!=="svm-historical-rpc/v1"||value.kind!=="ready"){
                        const error=value.error===undefined?undefined:object(value.error);
                        throw new ReplayError(typeof error?.code==="string"?error.code:"SESSION_PROTOCOL",typeof error?.message==="string"?error.message:"native fork did not become ready");
                    }
                    const url=new URL(String(value.url));
                    if(url.protocol!=="http:"||url.hostname!=="127.0.0.1"||url.username!==""||url.password!==""||url.search!==""||url.hash!==""||url.pathname!=="/")throw new ReplayError("SESSION_PROTOCOL","native fork endpoint must be loopback HTTP");
                    const pins=object(value.pins as Json);
                    if(pins.schema!=="svm-historical-session-pins/v1"||typeof pins.initialStateSha256!=="string")throw new ReplayError("SESSION_PROTOCOL","native fork provenance missing");
                    readySeen=true;readyResolve(value);
                } else if(closedSeen||value.kind!=="closed"||value.outcome!=="COMPLETED"||value.cleanup!=="OwnerReapedProcessTree") {
                    throw new ReplayError("SESSION_PROTOCOL","unexpected native fork terminal output");
                } else {closedSeen=true;}
            } catch(error){fail(error instanceof ReplayError?error:new ReplayError("SESSION_PROTOCOL",String(error)));}
        }
    });
    child.stderr.on("data",(data:Buffer)=>{bytes+=data.length;if(bytes>2*1024*1024)fail(new ReplayError("OUTPUT_LIMIT","historical fork diagnostics exceed 2MiB"));else stderr+=data.toString("utf8");});
    child.stdout.on("error",error=>fail(new ReplayError("PROCESS_ERROR",String(error))));
    child.stderr.on("error",error=>fail(new ReplayError("PROCESS_ERROR",String(error))));
    child.once("error",error=>fail(new ReplayError("PROCESS_ERROR","cannot start native historical fork",{cause:String(error)})));
    const done=new Promise<void>((resolve,reject)=>{
        child.once("close",(code,signal)=>{
            terminal=true;clearTimeout(timer);if(kill!==undefined)clearTimeout(kill);options.signal?.removeEventListener("abort",abort);
            if(failure===undefined&&(code!==0||signal!==null||!readySeen||!closedSeen||buffer!==""))failure=new ReplayError("SESSION_CLOSED","native historical fork exited unexpectedly",{code,signal,stderr});
            if(failure!==undefined){readyReject(failure);reject(failure);}else resolve();
        });
    });
    // Failures are surfaced by ready/operations/close, while a caller owns the session.
    const settled=done.then(()=>undefined,error=>{
        const failure=error as ReplayError;
        console.error(`historical fork stopped: ${failure.code}: ${failure.message}`);
        return failure;
    });
    let ready:JsonObject;
    try {ready=await readyPromise;}catch(error){await settled;throw error;}
    const url=ready.url as string,pins=object(ready.pins as Json);
    const wsUrl=typeof ready.wsUrl==="string"?ready.wsUrl:undefined;
    let id=0;
    async function rpc(method:string,params:Json[]=[]):Promise<Json> {
        if(failure!==undefined)throw failure;
        if(terminal)throw new ReplayError("SESSION_CLOSED","historical fork is closed");
        const requestId=++id;
        const response=await fetch(url,{method:"POST",headers:{"Content-Type":"application/json"},body:encodeJson({jsonrpc:"2.0",id:requestId,method,params}),signal:AbortSignal.timeout(30000)});
        if(response.status!==200)throw new ReplayError("SESSION_HTTP",`native RPC HTTP ${response.status}`);
        const text=await response.text();if(Buffer.byteLength(text)>2*1024*1024)throw new ReplayError("OUTPUT_LIMIT","fork-control response too large");
        const value=object(decodeJson(text));
        if(value.jsonrpc!=="2.0"||value.id!==requestId)throw new ReplayError("SESSION_PROTOCOL","fork-control response identity differs");
        if(value.error!==undefined){const error=object(value.error);throw new ReplayError("SESSION_RPC",typeof error.message==="string"?error.message:"native fork control rejected",{error});}
        if(value.result===undefined)throw new ReplayError("SESSION_PROTOCOL","fork-control result missing");
        return value.result;
    }
    return {url,wsUrl,pins,
        async setBalance(pubkey,lamports){return object(await rpc("svmReplay_setBalance",[pubkey,lamports.toString()]));},
        async overrideAccounts(entries){
            const resolved=await Promise.all(entries.map(resolveProgram));
            return object(await rpc("svmReplay_overrideAccounts",[resolved as unknown as Json[]]));
        },
        async advance(slots=1,options={}){
            if(!Number.isSafeInteger(slots)||slots<1)throw new ReplayError("INVALID_ADVANCE","slots must be a positive integer");
            const time=options.unixTimestamp===undefined?undefined:Number(options.unixTimestamp);
            if(time!==undefined&&!Number.isSafeInteger(time))throw new ReplayError("INVALID_ADVANCE","unixTimestamp must be a safe integer");
            const params:Json[]=time===undefined?[slots]:[slots,{unixTimestamp:time}];
            return object(await rpc("svmReplay_advance",params));
        },
        async snapshot(){const value=object(await rpc("svmReplay_snapshot"));if(!Number.isSafeInteger(value.forkId)||typeof value.forkId!=="number")throw new ReplayError("SESSION_PROTOCOL","fork id missing");return value.forkId;},
        async select(forkId){return object(await rpc("svmReplay_selectFork",[forkId]));},
        async reset(){return object(await rpc("svmReplay_reset"));},
        async closeFork(forkId){await rpc("svmReplay_closeFork",[forkId]);},
        async close(){if(!terminal){try{await rpc("svmReplay_close");}catch(error){fail(error instanceof ReplayError?error:new ReplayError("SESSION_RPC",String(error)));}}const error=await settled;if(error!==undefined)throw error;},
    };
}
