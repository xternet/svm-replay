import type {Replay} from "../client.js";
import {ReplayError,type SimulateOptions,type SignatureOptions,type Simulation} from "../types.js";
import type {ForkAtOptions} from "../session-options.js";
import type {HistoricalSession} from "../session.js";

/** Companion API: never installs historical state into Surfpool's current runtime. */
export function withHistoricalReplay<T extends object>(surfpool:T,replay:Replay):T & {simulateTransactionAt:(options:SimulateOptions|SignatureOptions)=>Promise<Simulation>;forkAt:(options:ForkAtOptions)=>Promise<HistoricalSession>} {
    if("simulateTransactionAt" in surfpool||"forkAt" in surfpool)throw new ReplayError("ADAPTER_CONFLICT","host already has historical replay methods");
    // A separate wrapper preserves the supplied instance's identity/lifecycle and native methods.
    return new Proxy(surfpool,{get(target,property,receiver){
        if(property==="simulateTransactionAt")return (options:SimulateOptions|SignatureOptions)=>"tx" in options?replay.replay(options):replay.simulate(options);
        if(property==="forkAt")return (options:ForkAtOptions)=>replay.forkAt(options);
        const value=Reflect.get(target,property,target);
        return typeof value==="function"?value.bind(target):value;
    }}) as T & {simulateTransactionAt:(options:SimulateOptions|SignatureOptions)=>Promise<Simulation>;forkAt:(options:ForkAtOptions)=>Promise<HistoricalSession>};
}
