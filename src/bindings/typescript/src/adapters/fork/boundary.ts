//! Native boundary transport. Historical rules and account derivation live in Rust.
import type {Replay} from "../../client.js";
import type {JsonObject,RuntimeIdentity,SignatureOptions} from "../../types.js";
import {ReplayError} from "../../types.js";
import {runJob} from "./jobs.js";
export interface AccountImage {pubkey:string;lamports:string;owner:string;executable:boolean;rentEpoch:string;dataBase64:string}
export interface BoundFeature {id:string;activationSlot:number}
export interface Boundary {
    signature:string;transactionBase64:string;family:string;parentSlot:number;targetSlot:number;blockHeight:number;epoch:number;
    accounts:Map<string,AccountImage>;absent:Set<string>;unavailable:Set<string>;
    clock:AccountImage;features:BoundFeature[];activeFeatures:string[];predecessors:number;
    expected:{original:JsonObject};receiptPath:string;runtime?:RuntimeIdentity|null;evidence:string[];
    featureImages:Map<string,AccountImage>;featureError:{code:string;message:string}|null;
}
function hydrate(value:JsonObject):Boundary {
    return {...value,accounts:new Map(Object.entries(value.accounts as unknown as Record<string,AccountImage>)),
        featureImages:new Map(Object.entries(value.featureImages as unknown as Record<string,AccountImage>)),
        absent:new Set(value.absent as string[]),unavailable:new Set(value.unavailable as string[])} as unknown as Boundary;
}
async function load(replay:Replay,options:SignatureOptions,extra:readonly string[],close:boolean):Promise<Boundary>{
    const jobs=[await runJob(replay,{...options,collect:{calls:true,maxEvents:100000}})];
    for(;;){
        const output=await replay.projectBoundary({schema:"svm-replay-boundary-request/v1",jobs,extra:[...extra]},options.signal);
        if(output.schema!=="svm-replay-boundary/v1")throw new ReplayError("HOST_PROTOCOL","native boundary schema differs");
        const boundary=hydrate(output.boundary as JsonObject);
        if(!close||output.next===null)return boundary;
        const next=output.next as {transactionBase64:string};
        jobs.push(await runJob(replay,{...options,collect:{calls:true,maxEvents:2},replace:{transactionBase64:next.transactionBase64}}));
    }
}
export function loadBoundary(replay:Replay,options:SignatureOptions):Promise<Boundary>{return load(replay,options,[],false);}
export function loadClosedBoundary(replay:Replay,options:SignatureOptions,extra:readonly string[]=[]):Promise<Boundary>{return load(replay,options,extra,true);}
