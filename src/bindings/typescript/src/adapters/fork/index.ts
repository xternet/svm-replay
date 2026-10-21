//! Serve the state just before a transaction as a local Solana RPC URL.
import type {Replay} from "../../client.js";
import type {SignatureOptions} from "../../types.js";
import {loadBoundary,type Boundary} from "./boundary.js";
import {serveBoundary,type Datasource} from "./datasource.js";
import {loadClosedBoundary} from "./boundary.js";
export {loadBoundary,serveBoundary};
export type {Boundary,Datasource};

export interface ForkOptions extends SignatureOptions {
    /** Server port; 0 picks a free port. */
    port?:number;
    /** Addresses the developer declares new, served as absent. */
    newAccounts?:string[];
    /** Historical accounts beyond the transaction's footprint. */
    also?:string[];
    /** Development only: serve unknown accounts as absent and report each assumption. */
    allowNewAccounts?:boolean;
}
export interface Fork {
    boundary:Boundary;datasource:Datasource;predecessors:number;close():Promise<void>;
}
/** Verify the transaction, close its boundary from engine evidence, and serve historical reads. */
export async function startHistorical(replay:Replay,options:ForkOptions):Promise<Fork>{
    const {port=0,newAccounts=[],also=[],allowNewAccounts=false,...signature}=options;
    const boundary=await loadClosedBoundary(replay,signature,also);
    const datasource=await serveBoundary(boundary,port,allowNewAccounts);
    for(const key of newAccounts)datasource.fresh.add(key);
    return {boundary,datasource,predecessors:boundary.predecessors,close:()=>datasource.close()};
}
