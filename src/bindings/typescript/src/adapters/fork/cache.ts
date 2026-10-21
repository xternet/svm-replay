//! Verified fork jobs, kept: the same request on the same pinned bundle and data settings gives the same verified
//! receipt, so a fork that starts again reuses its jobs instead of replaying the slot. A kept job is used only while its
//! receipt is byte-identical and every artifact still matches its pin; otherwise the job runs again.
import {createHash} from "node:crypto";
import {mkdir,readFile,rename,writeFile} from "node:fs/promises";
import {homedir} from "node:os";
import {dirname,join} from "node:path";
import type {Replay} from "../../client.js";
import type {Artifact,SignatureOptions} from "../../types.js";

const SCHEMA="svm-replay-fork-job/v1";
// Everything a result depends on; timeouts, callbacks and the key file do not change it.
const RESULT_FIELDS=["tx","replace","overrides","collect","fields","bankInputs","source","runtimeRegistry","maxRequests",
    "maxDownloadBytes","maxOutputBytes"] as const;
export interface KeptJob {schema:string;receiptPath:string;receiptSha256:string;trace:Artifact|null}

const sha=(bytes:Buffer|string)=>createHash("sha256").update(bytes).digest("hex");
/** A stable text for a JSON value: object keys sorted. */
const canonical=(value:unknown):string=>Array.isArray(value)?`[${value.map(canonical).join(",")}]`:value!==null&&typeof value==="object"?
    `{${Object.keys(value).sort().map(k=>`${JSON.stringify(k)}:${canonical((value as Record<string,unknown>)[k])}`).join(",")}}`:JSON.stringify(value)??"null";
/** The engine's default data directory: dirs::data_local_dir()/svm-replay. */
function defaultDataDir():string|undefined{
    if(process.platform==="win32")return process.env.LOCALAPPDATA===undefined?undefined:join(process.env.LOCALAPPDATA,"svm-replay");
    if(process.platform==="darwin")return join(homedir(),"Library","Application Support","svm-replay");
    return join(process.env.XDG_DATA_HOME||join(homedir(),".local","share"),"svm-replay");
}

/** Where this request's job is kept, or undefined: caching off, or an input given as a file (its contents decide). */
export function keptJobPath(replay:Replay,options:SignatureOptions):string|undefined{
    const scope=(replay as Partial<Replay>).forkJobScope?.();
    if(scope===undefined||options.noCache===true)return undefined;
    const request=Object.fromEntries(RESULT_FIELDS.filter(k=>options[k]!==undefined).map(k=>[k,options[k]]));
    if(canonical(request).includes('"path":'))return undefined;
    const directory=scope.dataDir??defaultDataDir();
    return directory===undefined?undefined:join(directory,"fork-jobs",`${sha(canonical({schema:SCHEMA,scope,request}))}.json`);
}
/** The kept job at `path` while its receipt is unchanged; undefined when there is none. */
export async function keptJob(path:string|undefined):Promise<KeptJob|undefined>{
    if(path===undefined)return undefined;
    const entry=await readFile(path,"utf8").then(text=>JSON.parse(text) as KeptJob,(error:NodeJS.ErrnoException)=>{
        if(error.code==="ENOENT")return undefined;throw error;});
    if(entry===undefined||entry.schema!==SCHEMA)return undefined;
    const receipt=await readFile(entry.receiptPath).catch(()=>undefined);  // removed with the data directory: run again
    return receipt!==undefined&&sha(receipt)===entry.receiptSha256?entry:undefined;
}
export async function keepJob(path:string|undefined,receiptPath:string,trace:Artifact|undefined):Promise<void>{
    if(path===undefined)return;
    const entry:KeptJob={schema:SCHEMA,receiptPath,receiptSha256:sha(await readFile(receiptPath)),trace:trace??null};
    await mkdir(dirname(path),{recursive:true,mode:0o700});
    const temporary=`${path}.${process.pid}.tmp`;
    await writeFile(temporary,JSON.stringify(entry),{mode:0o600});await rename(temporary,path);
}
