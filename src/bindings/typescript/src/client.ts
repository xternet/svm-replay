import {mkdtemp,writeFile,rm,realpath} from "node:fs/promises";
import {tmpdir} from "node:os";
import {resolve,join,dirname,basename} from "node:path";
import {isDeepStrictEqual} from "node:util";
import {bounded,digest,verifyArtifact,positive} from "./io.js";
import {decodeJson,encodeJson} from "./request.js";
import {run,type Execution} from "./process.js";
import {replaySignature} from "./signature.js";
import type {SignatureOptions,SignatureSimulation} from "./types.js";
import {ReplayError,type ReplayOptions,type SimulateOptions,type Simulation,type Receipt,type JsonObject} from "./types.js";

function response(execution:Execution):Receipt {
    const value=decodeJson(execution.stdout) as unknown as Receipt;
    if(value===null||typeof value!=="object"||typeof value.schema!=="string"||
        !["COMPLETED","NEEDS_INPUT","UNSUPPORTED","MISMATCH","CANCELLED","TIMEOUT","ERROR"].includes(value.outcome))
        throw new ReplayError("HOST_PROTOCOL","CLI did not return a typed outcome",{stderr:execution.stderr});
    if(execution.signal!==null||(value.outcome==="COMPLETED"?execution.exitCode!==0:execution.exitCode!==1))
        throw new ReplayError("HOST_PROTOCOL","CLI exit status differs from outcome",{exitCode:execution.exitCode,signal:execution.signal});
    return value;
}
function success(execution:Execution):Receipt {
    const value=response(execution);
    if(value.outcome!=="COMPLETED")throw new ReplayError(value.error?.code===undefined?"HOST_PROTOCOL":value.error.code,
        value.error?.message===undefined?"CLI rejected request":value.error.message,{response:value});
    return value;
}
export class Replay {
    private constructor(private readonly options:ReplayOptions,private readonly binary:string,private readonly binarySha256:string) {}
    /** Trust an already installed local bundle; no manually copied manifest hash required. */
    static async installed(directory:string):Promise<Replay>{
        const root=await realpath(resolve(directory));
        const pin=decodeJson((await bounded(join(root,"installation.json"),4096)).toString("utf8")) as JsonObject;
        if(pin.schema!=="svm-replay-installation-pin/v1"||typeof pin.bundleSha256!=="string")throw new ReplayError("BUNDLE_FORMAT","invalid installation pin");
        return Replay.open({bundlePath:join(root,"bundle.json"),bundleSha256:pin.bundleSha256});
    }
    /** Same --tx workflow as the CLI, including local .env loading and typed unsupported outcomes. */
    async replay(options:SignatureOptions):Promise<SignatureSimulation>{
        if(options.noCache===undefined&&this.options.cache==="prepared")throw new ReplayError("INVALID_REQUEST","signature replay supports all/off cache; set noCache explicitly");
        const settings={...options,source:options.source===undefined?this.options.source:options.source,
            alchemyConfig:options.alchemyConfig===undefined?this.options.alchemyConfig:options.alchemyConfig,
            noCache:options.noCache===undefined?this.options.cache==="off":options.noCache};
        return replaySignature(settings,this.binarySha256,this.options.dataDir,this.execute.bind(this),this.bundleArgs());
    }
    /** Bundle digest is supplied independently. There are no implicit network downloads. */
    static async open(options:ReplayOptions):Promise<Replay>{
        if(!["linux","darwin","win32"].includes(process.platform)||!["x64","arm64"].includes(process.arch))throw new ReplayError("UNSUPPORTED_PLATFORM","native workers require a supported OS/architecture and matching pinned bundle");
        if(!/^[a-f0-9]{64}$/.test(options.bundleSha256))throw new ReplayError("BUNDLE_INTEGRITY","invalid manifest digest");
        const bundlePath=await realpath(resolve(options.bundlePath));const bytes=await bounded(bundlePath,1024*1024);
        if(digest(bytes)!==options.bundleSha256)throw new ReplayError("BUNDLE_INTEGRITY","manifest hash differs");
        const manifest=decodeJson(bytes.toString("utf8")) as JsonObject;
        if(manifest.schema!=="svm-replay-bundle/v1"||typeof manifest.binary!=="string"||!Array.isArray(manifest.files))
            throw new ReplayError("BUNDLE_FORMAT","invalid bundle manifest");
        const entry=manifest.files.find(f=>f!==null&&typeof f==="object"&&!Array.isArray(f)&&f.path===manifest.binary) as JsonObject|undefined;
        if(entry===undefined||typeof entry.sha256!=="string"||entry.executable!==true)throw new ReplayError("BUNDLE_FORMAT","missing executable CLI");
        const verified=await verifyArtifact(dirname(bundlePath),{file:manifest.binary,sha256:entry.sha256},512*1024*1024);
        const client=new Replay({...options,bundlePath,dataDir:options.dataDir===undefined?undefined:resolve(options.dataDir),
            source:options.source===undefined?undefined:{...options.source,path:resolve(options.source.path)},
            alchemyConfig:options.alchemyConfig===undefined?undefined:resolve(options.alchemyConfig)},verified.path,verified.sha256);
        await client.capabilities();return client;
    }
    private bundleArgs():string[]{return ["--bundle",this.options.bundlePath,"--bundle-sha256",this.options.bundleSha256];}
    private async execute(args:string[],timeoutMs:number,signal?:AbortSignal,alchemy=false):Promise<Execution>{
        // Detect replacement before execution; Rust also binds its own bytes in every receipt.
        await verifyArtifact(dirname(this.binary),{file:basename(this.binary),sha256:this.binarySha256},512*1024*1024);
        return run(this.binary,args,{timeoutMs,maxBytes:16*1024*1024,signal,alchemy});
    }
    async capabilities():Promise<Receipt>{return success(await this.execute(["doctor",...this.bundleArgs()],30000));}
    async validate(requestPath:string):Promise<Receipt>{return success(await this.execute(["validate","--request",resolve(requestPath)],300000));}
    async simulate(options:SimulateOptions):Promise<Simulation>{
        const timeoutMs=options.timeoutMs===undefined?300000:options.timeoutMs;positive(timeoutMs,"timeoutMs");
        const start=performance.now(),directory=await mkdtemp(join(tmpdir(),"svm-replay-input-"));
        const remaining=()=>{const ms=Math.floor(timeoutMs-(performance.now()-start));if(ms<=0)throw new ReplayError("TIMEOUT","SDK request deadline exceeded");return ms;};
        const materialize=async(value:JsonObject|{path:string},name:string)=>{
            const bytes=Object.keys(value).length===1&&typeof value.path==="string"?await bounded(resolve(value.path),256*1024*1024):Buffer.from(encodeJson(value));
            const path=join(directory,name);await writeFile(path,bytes,{flag:"wx",mode:0o400});return path;
        };
        try {
            const request=await materialize(options.request,"request.json");
            const admitted=success(await this.execute(["validate","--request",request],remaining(),options.signal));
            const args=["simulate","--request",request,...this.bundleArgs()];
            if(this.options.dataDir!==undefined)args.push("--data-dir",this.options.dataDir);
            const source=options.source===undefined?this.options.source:options.source;
            const alchemy=options.alchemyConfig===undefined?this.options.alchemyConfig:options.alchemyConfig;
            const cache=options.cache===undefined?this.options.cache:options.cache;
            if(source!==undefined)args.push("--source",resolve(source.path),"--source-sha256",source.sha256);
            if(alchemy!==undefined)args.push("--alchemy",resolve(alchemy));
            if(cache!==undefined)args.push("--cache",cache);
            if(options.genesisHash!==undefined)args.push("--genesis-hash",options.genesisHash);
            if(options.trace!==undefined)args.push("--trace",await materialize(options.trace,"trace.json"));
            const execution=await this.execute(args,remaining(),options.signal,alchemy!==undefined);
            const receipt=response(execution);
            // A pre-admission failure has no durable job. Preserve it without inventing evidence.
            if(receipt.schema==="svm-replay-cli-error/v1")return {receipt,stderr:execution.stderr};
            if(receipt.schema!=="svm-replay-receipt/v1"||typeof receipt.receiptPath!=="string"||receipt.requestId!==admitted.requestId
                ||receipt.requestIdentity!==admitted.requestIdentity||receipt.implementationSha256!==this.binarySha256
                ||receipt.processOwnerSha256!==this.binarySha256||receipt.currentStateFallback!==false)
                throw new ReplayError("RECEIPT_INTEGRITY","receipt identity differs from admitted request/CLI");
            const {receiptPath,...stored}=receipt;
            const disk=decodeJson((await bounded(receiptPath,16*1024*1024)).toString("utf8"));
            if(!isDeepStrictEqual(disk,stored))throw new ReplayError("RECEIPT_INTEGRITY","stdout and durable receipt differ");
            if(receipt.outcome!=="COMPLETED"){
                if(receipt.output!==undefined)throw new ReplayError("RECEIPT_INTEGRITY","non-completed outcome has output");
                return {receipt,stderr:execution.stderr};
            }
            if(receipt.controlVerification?.status!=="PASS"||receipt.verification?.status!=="PASS"||receipt.output===undefined)
                throw new ReplayError("RECEIPT_INTEGRITY","completion lacks verified original control/result");
            const limits=admitted.limits as {maxOutputBytes:number};
            const result=await verifyArtifact(dirname(receiptPath),receipt.output,limits.maxOutputBytes);
            return {receipt,result,stderr:execution.stderr};
        } finally {await rm(directory,{recursive:true,force:false});}
    }
}
