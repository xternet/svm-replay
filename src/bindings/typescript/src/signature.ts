//! Signature convenience delegates all reconstruction to the native CLI.
import {mkdtemp,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join,dirname,resolve} from "node:path";
import {isDeepStrictEqual} from "node:util";
import {bounded,positive} from "./io.js";
import {materializeSignatureInputs} from "./signature-input.js";
import {verifySignatureOutput} from "./signature-output.js";
import {decodeJson,encodeJson} from "./request.js";
import type {Execution} from "./process.js";
import {ReplayError,type SignatureOptions,type SignatureSimulation,type Receipt,type JsonObject} from "./types.js";

export function signatureArgs(options:SignatureOptions,out:string):string[]{
    if("trace" in options)throw new ReplayError("INVALID_REQUEST","use collect instead of the removed signature trace option");
    if(options.alchemyConfig!==undefined&&(options.maxRequests!==undefined||options.maxDownloadBytes!==undefined))
        throw new ReplayError("INVALID_REQUEST","alchemyConfig cannot be combined with maxRequests/maxDownloadBytes");
    if(!/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(options.tx))throw new ReplayError("INVALID_REQUEST","invalid transaction signature");
    const args=["--tx",options.tx,"--out",out];
    const input=(value:unknown)=>{
        if(value!==null&&typeof value==="object"&&!Array.isArray(value)&&Object.keys(value).length===1&&"path" in value&&typeof value.path==="string")return "@"+resolve(value.path);
        return encodeJson(value as JsonObject);
    };
    if(options.replace!==undefined)args.push("--replace",input(options.replace));
    if(options.overrides!==undefined)args.push("--overrides",input(options.overrides));
    if(options.collect!==undefined)args.push("--collect",options.collect==="all"?"all":input(options.collect));
    if(options.fields!==undefined)args.push("--fields",options.fields.join(","));
    if(options.noCache)args.push("--no-cache");
    if(options.alchemyKeyFile!==undefined)args.push("--alchemy-key-file",resolve(options.alchemyKeyFile));
    if(options.alchemyConfig!==undefined)args.push("--alchemy-config",resolve(options.alchemyConfig));
    for(const [flag,value] of [["--timeout",options.timeoutMs===undefined?undefined:Math.ceil(options.timeoutMs/1000)],
        ["--max-requests",options.maxRequests],["--max-download-bytes",options.maxDownloadBytes],["--max-output-bytes",options.maxOutputBytes]] as const){
        if(value!==undefined){positive(value,flag);args.push(flag,String(value));}
    }
    for(const name of options.bankInputs===undefined?[]:options.bankInputs)args.push("--bank-input",name);
    for(const [flag,source] of [["--source",options.source],["--runtime-registry",options.runtimeRegistry]] as const){
        if(source!==undefined)args.push(flag,resolve(source.path),flag+"-sha256",source.sha256);
    }
    return args;
}
export async function replaySignature(options:SignatureOptions,binaryHash:string,dataDir:string|undefined,
    execute:(args:string[],timeout:number,signal?:AbortSignal,alchemy?:boolean)=>Promise<Execution>,bundleArgs:readonly string[]=[]):Promise<SignatureSimulation>{
    const timeout=options.timeoutMs===undefined?900000:options.timeoutMs;positive(timeout,"timeoutMs");
    const directory=await mkdtemp(join(tmpdir(),"svm-signature-"));
    try {
        const args=signatureArgs(await materializeSignatureInputs(options,directory),directory);
        if(dataDir!==undefined)args.push("--data-dir",dataDir);
        args.push(...bundleArgs);
        const execution=await execute(args,timeout+10000,options.signal,true);
        const response=decodeJson(execution.stdout) as unknown as Receipt;
        if(response===null||typeof response!=="object"||!["COMPLETED","NEEDS_INPUT","UNSUPPORTED","MISMATCH","CANCELLED","TIMEOUT","ERROR"].includes(response.outcome)||
            execution.signal!==null||execution.exitCode!==(response.outcome==="COMPLETED"?0:1))throw new ReplayError("HOST_PROTOCOL","CLI outcome or exit status differs");
        if(response.schema==="svm-replay-cli-error/v1"){
            if(response.savedTo===undefined)return {receipt:response,stderr:execution.stderr};
            if(typeof response.savedTo!=="string"||dirname(response.savedTo)!==directory)throw new ReplayError("RECEIPT_INTEGRITY","saved error escaped output directory");
            const error=decodeJson((await bounded(response.savedTo,16*1024*1024)).toString("utf8")) as unknown as Receipt;
            if(!isDeepStrictEqual(error,{schema:response.schema,outcome:response.outcome,error:response.error}))throw new ReplayError("RECEIPT_INTEGRITY","saved admission error differs");
            return {receipt:error,stderr:execution.stderr};
        }
        if(typeof response.savedTo!=="string"||dirname(response.savedTo)!==directory)throw new ReplayError("RECEIPT_INTEGRITY","saved response escaped output directory");
        const full=decodeJson((await bounded(response.savedTo,1024*1024*1024)).toString("utf8")) as JsonObject;
        if(typeof full.receiptPath!=="string"||full.requestId!==`tx-${options.tx}`||full.implementationSha256!==binaryHash||
            full.processOwnerSha256!==binaryHash||full.currentStateFallback!==false)throw new ReplayError("RECEIPT_INTEGRITY","signature receipt identity differs");
        const stored=decodeJson((await bounded(full.receiptPath,16*1024*1024)).toString("utf8")) as unknown as Receipt;
        const comparable=structuredClone(full);delete comparable.receiptPath;delete comparable.result;
        const trace=comparable.trace;
        if(trace!==null&&typeof trace==="object"&&!Array.isArray(trace)&&Array.isArray(trace.exports)){
            for(const exp of trace.exports){if(exp!==null&&typeof exp==="object"&&!Array.isArray(exp))delete exp.data;}
        }
        if(!isDeepStrictEqual(stored,comparable)||stored.outcome!==response.outcome)throw new ReplayError("RECEIPT_INTEGRITY","saved and durable receipts differ");
        const receipt={...stored,receiptPath:full.receiptPath};
        const result=await verifySignatureOutput(full,receipt,dirname(full.receiptPath),options.fields===undefined?[]:options.fields,options.maxOutputBytes===undefined?256*1024*1024:options.maxOutputBytes);
        if(receipt.outcome!=="COMPLETED")return {receipt,stderr:execution.stderr,response:full};
        if(receipt.controlVerification?.status!=="PASS"||receipt.verification?.status!=="PASS"||receipt.output===undefined)throw new ReplayError("RECEIPT_INTEGRITY","completion lacks verification");
        return {receipt,result,stderr:execution.stderr,response:full};
    } finally {await rm(directory,{recursive:true,force:false});}
}
