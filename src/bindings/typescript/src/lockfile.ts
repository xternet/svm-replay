import {open,readFile,rename,rm,unlink,writeFile} from "node:fs/promises";
import {randomUUID} from "node:crypto";
import {resolve} from "node:path";
import {ReplayError,type JsonObject} from "./types.js";

const SCHEMA="svm-replay-lock/v1";

/** What a fork resolved to, independent of platform and worker build. The runtime
 *  family names the pinned historical runtime (one Agave release per family). */
export function lockEntry(pins:JsonObject):JsonObject {
    const network=pins.genesisHash,runtime=pins.family;
    const slot=pins.slot,transactionIndex=pins.transactionIndex,signature=pins.signature;
    if(typeof network!=="string"||network===""||typeof runtime!=="string"||runtime==="")
        throw new ReplayError("LOCKFILE","fork pins lack network or runtime");
    if(typeof slot!=="number"||!Number.isSafeInteger(slot)||slot<0||
       typeof transactionIndex!=="number"||!Number.isSafeInteger(transactionIndex)||transactionIndex<0)
        throw new ReplayError("LOCKFILE","fork pins have invalid slot or transaction index");
    if(signature!==null&&(typeof signature!=="string"||signature===""))
        throw new ReplayError("LOCKFILE","fork pins lack a transaction signature or explicit null");
    return {network,slot,transactionIndex,transaction:signature,runtime};
}

/** Record a fork the first time; afterwards, fail when it resolves differently. */
export async function checkLock(path:string,after:string,pins:JsonObject):Promise<void> {
    const target=resolve(path),guard=`${target}.lock`;
    const handle=await acquire(guard);
    try {await update(target,after,pins);}
    finally {try {await handle.close();}finally {await unlink(guard);}}
}

async function acquire(path:string){
    const deadline=Date.now()+30000;
    for(;;){
        try {return await open(path,"wx",0o600);}
        catch(error){
            if((error as NodeJS.ErrnoException).code!=="EEXIST")
                throw new ReplayError("LOCKFILE",`${path}: cannot acquire writer lock`,{cause:String(error)});
            if(Date.now()>=deadline)throw new ReplayError("LOCKFILE_BUSY",`${path}: writer lock is still held; check for an interrupted writer`);
            await new Promise(resolve=>setTimeout(resolve,10));
        }
    }
}

async function update(path:string,after:string,pins:JsonObject):Promise<void> {
    let lock:{schema:string;forks:Record<string,JsonObject>};
    try {lock=JSON.parse(await readFile(path,"utf8"));}
    catch(error){
        if((error as NodeJS.ErrnoException).code!=="ENOENT")throw new ReplayError("LOCKFILE",`${path}: ${String(error)}`);
        lock={schema:SCHEMA,forks:{}};
    }
    if(lock===null||lock.schema!==SCHEMA||lock.forks===null||typeof lock.forks!=="object"||Array.isArray(lock.forks))
        throw new ReplayError("LOCKFILE",`${path} is not an ${SCHEMA} file`);
    const entry=lockEntry(pins),recorded=lock.forks[after];
    if(recorded!==undefined){
        const differs=Object.keys(entry).filter(key=>JSON.stringify(recorded[key])!==JSON.stringify(entry[key]));
        if(differs.length>0)throw new ReplayError("LOCK_MISMATCH",`${after} resolved differently than ${path} records: ${differs.join(", ")}`,{recorded,resolved:entry});
        return;
    }
    lock.forks[after]=entry;
    const temporary=`${path}.${process.pid}.${randomUUID()}.tmp`;
    try {
        await writeFile(temporary,`${JSON.stringify(lock,null,2)}\n`,{flag:"wx",mode:0o600});
        await rename(temporary,path);
    } finally {await rm(temporary,{force:true});}
}
