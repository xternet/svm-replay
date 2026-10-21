//! Job transport and cache integrity only; native code interprets prepared/result/journal bytes.
import {readFile} from "node:fs/promises";
import {dirname} from "node:path";
import type {Replay} from "../../client.js";
import {digest,readArtifact} from "../../io.js";
import {ReplayError,type Artifact,type JsonObject,type Receipt,type SignatureOptions} from "../../types.js";
import {keepJob,keptJob,keptJobPath} from "./cache.js";
async function descriptor(receipt:Receipt,trace:Artifact|undefined):Promise<JsonObject>{
    if(trace===undefined)throw new ReplayError("BOUNDARY_FORMAT","original journal pin missing");
    const path=receipt.receiptPath as string,root=dirname(path);
    for(const artifact of [receipt.preparedRequest,receipt.output,trace])await readArtifact(root,artifact as Artifact,256*1024*1024);
    return {receiptPath:path,receiptSha256:digest(await readFile(path)),trace:{file:trace.file,sha256:trace.sha256}};
}
export async function runJob(replay:Replay,options:SignatureOptions):Promise<JsonObject>{
    const path=keptJobPath(replay,options),kept=await keptJob(path);
    if(kept!==undefined){
        try {
            const receipt=JSON.parse(await readFile(kept.receiptPath,"utf8")) as Receipt;
            return await descriptor({...receipt,receiptPath:kept.receiptPath},kept.trace===null?undefined:kept.trace);
        }catch(error){console.warn("Discarding invalid cached fork job",{receiptPath:kept.receiptPath,error:String(error)});}
    }
    const {receipt,response}=await replay.replay(options);
    if(receipt.outcome!=="COMPLETED"){
        const error=receipt.error,details=error?.details as {reason?:unknown;pubkey?:unknown}|undefined;
        const hint=details?.reason==="unproven-account-absence"?` ${String(details.pubkey)} has no account at that moment in the `+
            "archive, and its absence is not provable from the block: if it did not exist yet, declare it with --new-account.":"";
        throw new ReplayError("BOUNDARY_UNVERIFIED",`SVM Replay could not verify ${options.tx}: ${receipt.outcome}`+
            `${error?.message===undefined?"":` (${error.message})`}.${hint}`,{receipt});
    }
    const pinned=((response?.trace as JsonObject|undefined)?.exports as JsonObject[]|undefined)?.[0] as unknown as Artifact|undefined;
    const job=await descriptor(receipt,pinned);
    await keepJob(path,receipt.receiptPath as string,pinned);
    return job;
}
