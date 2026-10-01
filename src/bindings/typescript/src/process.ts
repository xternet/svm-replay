import {spawn} from "node:child_process";
import {ReplayError} from "./types.js";
import {positive} from "./io.js";

export interface ProcessOptions {signal?:AbortSignal;timeoutMs:number;maxBytes:number;alchemy:boolean}
export interface Execution {stdout:string;stderr:string;exitCode:number|null;signal:NodeJS.Signals|null}
export function environment(alchemy:boolean):NodeJS.ProcessEnv {
    const env:NodeJS.ProcessEnv={};
    for(const name of ["PATH","HOME","USERPROFILE","LOCALAPPDATA","XDG_DATA_HOME","TMPDIR","SystemRoot","WINDIR","TEMP","TMP"]){const v=process.env[name];if(v!==undefined)env[name]=v;}
    if(alchemy){const key=process.env.API_ALCHEMY;if(key!==undefined)env.API_ALCHEMY=key;}
    return env;
}
/** Rust owns/reaps its worker tree; the SDK waits for close, including cancellation. */
export async function run(binary:string,args:string[],options:ProcessOptions):Promise<Execution>{
    positive(options.timeoutMs,"timeoutMs");positive(options.maxBytes,"maxBytes");
    if(options.signal?.aborted)throw new ReplayError("CANCELLED","cancelled before starting CLI");
    const child=spawn(binary,args,{shell:false,stdio:["ignore","pipe","pipe"],env:environment(options.alchemy)});
    const stdout:Buffer[]=[],stderr:Buffer[]=[];let size=0,failure:ReplayError|undefined,kill:ReturnType<typeof setTimeout>|undefined;
    const fail=(error:ReplayError)=>{if(failure!==undefined)return;failure=error;child.kill("SIGTERM");kill=setTimeout(()=>child.kill("SIGKILL"),10000);};
    const collect=(chunk:Buffer,into:Buffer[])=>{size+=chunk.length;if(size>options.maxBytes){fail(new ReplayError("DIAGNOSTIC_LIMIT","CLI response exceeds configured bound"));return;}into.push(chunk);};
    child.stdout.on("data",c=>collect(c,stdout));child.stderr.on("data",c=>collect(c,stderr));
    child.stdout.on("error",e=>fail(new ReplayError("PROCESS_ERROR",String(e))));child.stderr.on("error",e=>fail(new ReplayError("PROCESS_ERROR",String(e))));
    const abort=()=>fail(new ReplayError("CANCELLED","simulation cancelled"));
    options.signal?.addEventListener("abort",abort,{once:true});
    if(options.signal?.aborted)abort();
    const timer=setTimeout(()=>fail(new ReplayError("TIMEOUT","CLI deadline exceeded")),options.timeoutMs);
    return new Promise((done,reject)=>{
        child.once("error",e=>fail(new ReplayError("PROCESS_ERROR","could not start CLI",{cause:String(e)})));
        child.once("close",(exitCode,signal)=>{clearTimeout(timer);if(kill!==undefined)clearTimeout(kill);options.signal?.removeEventListener("abort",abort);
            const result={stdout:Buffer.concat(stdout).toString("utf8"),stderr:Buffer.concat(stderr).toString("utf8"),exitCode,signal};
            if(failure!==undefined){Object.assign(failure.details,result);reject(failure);}else done(result);});
    });
}
