import {writeFile} from "node:fs/promises";
import {join,resolve} from "node:path";
import {bounded} from "./io.js";
import {encodeJson} from "./request.js";
import type {SignatureOptions} from "./types.js";

/** Snapshot inputs into private files; argv limits must not constrain account/program size. */
export async function materializeSignatureInputs(options:SignatureOptions,directory:string):Promise<SignatureOptions>{
    const result={...options};
    for(const field of ["replace","overrides","collect"] as const){
        const value=options[field];
        if(value===undefined||value==="all")continue;
        const bytes=!Array.isArray(value)&&Object.keys(value).length===1&&typeof value.path==="string"
            ?await bounded(resolve(value.path),16*1024*1024):Buffer.from(encodeJson(value));
        const path=join(directory,field+".json");
        await writeFile(path,bytes,{flag:"wx",mode:0o400});
        result[field]={path};
    }
    return result;
}
