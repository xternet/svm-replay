import {positive} from "./io.js";
import {environment} from "./process.js";
import {ReplayError,type Source} from "./types.js";
import {resolve} from "node:path";

export interface ForkAtOptions {
    after:string; provider?:string; config?:string; key?:string; keyFile?:string;
    /** Additional environment references used by your provider TOML; values never enter arguments/pins. */
    credentialEnv?:readonly string[];
    timeoutMs?:number; signal?:AbortSignal; maxRequests?:number; maxDownloadBytes?:number;
    runtimeRegistry?:Source;
    /** Record the resolved fork in this JSON file (created if absent); a later run that resolves differently fails. */
    lockfile?:string;
}

/** CLI argument plumbing only. Rust parses selectors and resolves providers/runtimes. */
export function sessionArguments(options:ForkAtOptions,bundleArgs:string[],dataDir:string|undefined):{args:string[];env:NodeJS.ProcessEnv;timeoutMs:number} {
    const timeoutMs=options.timeoutMs===undefined?3600000:options.timeoutMs;
    positive(timeoutMs,"timeoutMs");
    if(timeoutMs>3600000)throw new ReplayError("INVALID_LIMIT","native session lifetime cannot exceed one hour");
    if(options.key!==undefined&&options.keyFile!==undefined)throw new ReplayError("INVALID_REQUEST","choose key or keyFile");
    const args=["fork","--after",options.after,"--port","0",...bundleArgs,"--timeout",String(Math.ceil(timeoutMs/1000))];
    if(process.platform==="linux")args.push("--parent-pid",String(process.pid));
    const config=options.config===undefined?process.env.SVM_REPLAY_HISTORICAL_CONFIG:options.config;
    if(dataDir!==undefined)args.push("--data-dir",dataDir);
    if(options.provider!==undefined)args.push("--provider",options.provider);
    if(config!==undefined)args.push("--config",resolve(config));
    if(options.key!==undefined)args.push("--key",options.key.startsWith("env:")?options.key:"env:SVM_REPLAY_SESSION_KEY");
    if(options.keyFile!==undefined)args.push("--key-file",resolve(options.keyFile));
    if(options.runtimeRegistry!==undefined)args.push("--runtime-registry",resolve(options.runtimeRegistry.path),"--runtime-registry-sha256",options.runtimeRegistry.sha256);
    for(const [name,value] of [["max-requests",options.maxRequests],["max-download-bytes",options.maxDownloadBytes]] as const){
        if(value!==undefined){positive(value,name);args.push(`--${name}`,String(value));}
    }
    const env=environment(true);
    const references=[...(options.credentialEnv===undefined?[]:options.credentialEnv)];
    if(options.key?.startsWith("env:"))references.push(options.key.slice(4));
    for(const name of references){
        if(!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name))throw new ReplayError("INVALID_REQUEST","invalid credential environment reference");
        if(process.env[name]!==undefined)env[name]=process.env[name];
    }
    if(options.key!==undefined&&!options.key.startsWith("env:"))env.SVM_REPLAY_SESSION_KEY=options.key;
    return {args,env,timeoutMs};
}
