import {constants} from "node:fs";
import {Buffer} from "node:buffer";
import {open,lstat,realpath} from "node:fs/promises";
import {resolve,join,isAbsolute} from "node:path";
import {createHash} from "node:crypto";
import {ReplayError, type Artifact, type ArtifactHandle} from "./types.js";

export function positive(value: number, label: string): void {
    if (!Number.isSafeInteger(value) || value <= 0) throw new ReplayError("INVALID_LIMIT", `${label} must be a positive safe integer`);
}
export function digest(bytes: Uint8Array): string { return createHash("sha256").update(bytes).digest("hex"); }
export async function bounded(path: string, max: number): Promise<Buffer> {
    positive(max,"maxBytes");
    const handle=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW);
    try {
        const info=await handle.stat();
        if (!info.isFile()) throw new ReplayError("ARTIFACT_PATH","expected regular file",{path});
        if (info.size>max) throw new ReplayError("OUTPUT_LIMIT","file exceeds read limit",{path,bytes:info.size,max});
        const chunks:Buffer[]=[];let size=0;
        for (;;) { const chunk=Buffer.allocUnsafe(Math.min(65536,max-size+1));const r=await handle.read(chunk);
            if(r.bytesRead===0) break;size+=r.bytesRead;
            if(size>max) throw new ReplayError("OUTPUT_LIMIT","file grew past read limit",{path,max});
            chunks.push(chunk.subarray(0,r.bytesRead)); }
        return Buffer.concat(chunks,size);
    } finally {await handle.close();}
}
export async function confined(root: string, relative: string): Promise<string> {
    if(typeof relative!=="string"||isAbsolute(relative)||relative.includes("\\")||relative.split("/").some(p=>p===""||p==="."||p===".."))
        throw new ReplayError("ARTIFACT_PATH","expected confined relative path");
    let path=await realpath(resolve(root));
    for(const part of relative.split("/")){path=join(path,part);const stat=await lstat(path);
        if(stat.isSymbolicLink())throw new ReplayError("ARTIFACT_PATH","symlink in artifact path",{path});}
    return path;
}
/** Stream verification: callers need not load a large trace/result into JS memory. */
export async function verifyArtifact(root: string, artifact: Artifact, max: number): Promise<ArtifactHandle> {
    positive(max,"maxBytes");
    if(!/^[a-f0-9]{64}$/.test(artifact.sha256))throw new ReplayError("ARTIFACT_INTEGRITY","invalid digest");
    const path=await confined(root,artifact.file),stat=await lstat(path);
    if(!stat.isFile())throw new ReplayError("ARTIFACT_PATH","not a regular file",{path});
    if(stat.size>max)throw new ReplayError("OUTPUT_LIMIT","artifact exceeds limit",{path,bytes:stat.size,max});
    const hash=createHash("sha256");let bytes=0;
    const handle=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW);
    const stream=handle.createReadStream({autoClose:false});
    try {for await(const chunk of stream){bytes+=chunk.length;if(bytes>max)throw new ReplayError("OUTPUT_LIMIT","artifact grew past limit",{path,max});hash.update(chunk);}}
    finally {stream.destroy();await handle.close();}
    if(hash.digest("hex")!==artifact.sha256)throw new ReplayError("ARTIFACT_INTEGRITY","artifact hash differs",{path});
    return {...artifact,path,bytes};
}
export async function readArtifact(root: string, artifact: Artifact, max: number): Promise<Buffer> {
    const path=await confined(root,artifact.file);const bytes=await bounded(path,max);
    if(digest(bytes)!==artifact.sha256)throw new ReplayError("ARTIFACT_INTEGRITY","artifact hash differs",{path});
    return bytes;
}
