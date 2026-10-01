import {isDeepStrictEqual} from "node:util";
import {confined,readArtifact} from "./io.js";
import {decodeJson} from "./request.js";
import {ReplayError,type Artifact,type ArtifactHandle,type JsonObject,type Receipt} from "./types.js";

/** Returned JSON must match the verified bytes, not merely a matching receipt header. */
export async function verifySignatureOutput(full:JsonObject,receipt:Receipt,root:string,fields:readonly string[],max:number):Promise<ArtifactHandle|undefined>{
    const fail=(message:string):never=>{throw new ReplayError("RECEIPT_INTEGRITY",message);};
    if(receipt.outcome!=="COMPLETED"){
        if(receipt.output!==undefined||full.result!==undefined)fail("non-completed outcome has result");
        return undefined;
    }
    if(receipt.output===undefined)fail("completed result missing");
    const output=receipt.output as Artifact;
    const bytes=await readArtifact(root,output,max);
    const expected=decodeJson(bytes.toString("utf8")) as JsonObject;
    if(fields.length>0){
        for(const name of ["original","replacement","accountOverride","requestedOverrides"]){
            const tx=expected[name];
            if(tx!==null&&typeof tx==="object"&&!Array.isArray(tx))for(const key of Object.keys(tx))if(!fields.includes(key))delete tx[key];
        }
    }
    if(!isDeepStrictEqual(full.result,expected))fail("returned result differs from verified artifact");
    const trace=full.trace;
    if(trace!==undefined){
        if(trace===null||typeof trace!=="object"||Array.isArray(trace)||!Array.isArray(trace.exports))fail("invalid trace exports");
        const exports=(trace as JsonObject).exports as JsonObject[];
        const wanted=fields.length===0||fields.includes("trace");
        for(const entry of exports){
            if(!wanted){if(entry.data!==undefined)fail("unselected trace payload returned");continue;}
            if(typeof entry.file!=="string"||typeof entry.sha256!=="string")fail("trace pin missing");
            const payload=await readArtifact(root,{file:entry.file as string,sha256:entry.sha256 as string},256*1024*1024);
            if(!isDeepStrictEqual(entry.data,decodeJson(payload.toString("utf8"))))fail("returned trace differs from verified artifact");
        }
    }
    return {...output,path:await confined(root,output.file),bytes:bytes.length};
}
