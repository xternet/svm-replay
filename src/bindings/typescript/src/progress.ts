import {StringDecoder} from "node:string_decoder";
import {ReplayError,type ProgressEvent,type Outcome} from "./types.js";

const phases=["locating_transaction","resolving_dependencies","fetching_historical_accounts",
    "replaying_earlier_transactions","verifying_original","saving_inputs","executing_requested"];
const outcomes=["COMPLETED","NEEDS_INPUT","UNSUPPORTED","MISMATCH","CANCELLED","TIMEOUT","ERROR"];
const integer=(value:unknown):value is number=>typeof value==="number"&&Number.isSafeInteger(value)&&value>=0;
function validate(value:Record<string,unknown>):ProgressEvent {
    const invalid=()=>new ReplayError("HOST_PROTOCOL","malformed native progress event");
    if(value.origin!=="native"||!phases.includes(String(value.phase))||
        !["phase","advance","heartbeat","terminal"].includes(String(value.kind)))throw invalid();
    for(const field of ["sequence","elapsedMs","phaseDurationMs","sinceAdvanceMs","attempt"])
        if(!integer(value[field]))throw invalid();
    for(const field of ["fetchedAccounts","cacheHits","downloadedBytes","processedEarlierTransactions"])
        if(value[field]!==undefined&&!integer(value[field]))throw invalid();
    if(value.downloadedBytesKnown!==undefined&&typeof value.downloadedBytesKnown!=="boolean")throw invalid();
    if(value.earlierTransactionsTotal!==undefined){
        const total=value.earlierTransactionsTotal as Record<string,unknown>;
        if(total===null||typeof total!=="object"||!integer(total.value)||!["provisional","final"].includes(String(total.status)))throw invalid();
    }
    if(value.kind==="terminal"?!outcomes.includes(String(value.outcome)):value.outcome!==undefined)throw invalid();
    if((value.phaseDurationMs as number)>(value.elapsedMs as number)||(value.sinceAdvanceMs as number)>(value.elapsedMs as number))throw invalid();
    // Forward only the public numeric/enum schema, never arbitrary provider payloads.
    const event:Record<string,unknown>={};
    for(const field of ["schema","origin","sequence","kind","phase","elapsedMs","phaseDurationMs","sinceAdvanceMs",
        "attempt","fetchedAccounts","cacheHits","downloadedBytes","downloadedBytesKnown","processedEarlierTransactions",
        "earlierTransactionsTotal","outcome"]){if(value[field]!==undefined)event[field]=value[field];}
    if(value.earlierTransactionsTotal!==undefined){
        const total=value.earlierTransactionsTotal as Record<string,unknown>;
        event.earlierTransactionsTotal={value:total.value,status:total.status};
    }
    return event as unknown as ProgressEvent;
}

/** Bounded NDJSON framing; progress is not accumulated as diagnostics. */
export class ProgressStream {
    private readonly decoder=new StringDecoder("utf8");
    private pending="";
    private last?:ProgressEvent;
    private receivedAt=performance.now();
    private readonly started=performance.now();
    private callbackFailed=false;
    constructor(private readonly callback:(event:ProgressEvent)=>void,
        private readonly diagnostic:(chunk:Buffer)=>void,private readonly maxLineBytes:number) {}
    feed(chunk:Buffer):void {this.pending+=this.decoder.write(chunk);this.lines();}
    end():void {
        this.pending+=this.decoder.end();this.lines();
        if(this.pending!==""){this.line(this.pending);this.pending="";}
    }
    private lines():void {
        let end:number;
        while((end=this.pending.indexOf("\n"))>=0){
            const line=this.pending.slice(0,end+1);this.pending=this.pending.slice(end+1);this.line(line);
        }
        if(Buffer.byteLength(this.pending)>this.maxLineBytes)throw new ReplayError("DIAGNOSTIC_LIMIT","CLI stderr line exceeds configured bound");
    }
    private line(line:string):void {
        if(Buffer.byteLength(line)>this.maxLineBytes)throw new ReplayError("DIAGNOSTIC_LIMIT","CLI stderr line exceeds configured bound");
        let value:unknown;
        try {value=JSON.parse(line);}catch{this.diagnostic(Buffer.from(line));return;}
        if(value===null||typeof value!=="object"||Array.isArray(value)||(value as Record<string,unknown>).schema!=="svm-replay-progress/v1"){
            this.diagnostic(Buffer.from(line));return;
        }
        if(Buffer.byteLength(line)>65536)throw new ReplayError("HOST_PROTOCOL","native progress line exceeds protocol bound");
        const event=validate(value as Record<string,unknown>);
        if(this.last?.kind==="terminal"||event.sequence!==(this.last===undefined?1:this.last.sequence+1)||
            (this.last!==undefined&&event.elapsedMs<this.last.elapsedMs))throw new ReplayError("HOST_PROTOCOL","native progress ordering differs");
        this.last=event;this.receivedAt=performance.now();this.deliver(event);
    }
    private deliver(event:ProgressEvent):void {
        if(this.callbackFailed)return;
        try {this.callback(event);}catch{
            this.callbackFailed=true;
            throw new ReplayError("PROGRESS_CALLBACK","onProgress callback threw");
        }
    }
    terminal(outcome:Outcome):void {
        if(this.last?.kind==="terminal"||this.callbackFailed)return;
        const now=performance.now(),delta=Math.floor(now-this.receivedAt);
        const elapsedMs=this.last===undefined?Math.floor(now-this.started):this.last.elapsedMs+delta;
        const event:ProgressEvent={...this.last,schema:"svm-replay-progress/v1",origin:"sdk",kind:"terminal",outcome,
            phase:this.last?.phase,
            sequence:this.last===undefined?1:this.last.sequence+1,elapsedMs,
            phaseDurationMs:this.last?.phaseDurationMs===undefined?undefined:this.last.phaseDurationMs+delta,
            sinceAdvanceMs:this.last===undefined?elapsedMs:this.last.sinceAdvanceMs+delta,
            attempt:this.last===undefined?0:this.last.attempt};
        this.last=event;this.deliver(event);
    }
}
