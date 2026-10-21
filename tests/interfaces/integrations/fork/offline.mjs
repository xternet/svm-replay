/** Native prepared-input transport for opt-in cached-source drivers. No provider or replay logic lives here. */
import {readFile} from "node:fs/promises";
import {dirname} from "node:path";
import {createHash} from "node:crypto";

export async function preparedJob(context,options){
    let receiptPath;
    if(process.env.SVM_REPLAY_FORK_INPUTS!==undefined){
        // Reviewed input metadata, not results: the new native engine still reconstructs and executes every request.
        const inputs=JSON.parse(await readFile(process.env.SVM_REPLAY_FORK_INPUTS,"utf8")),entry=inputs[options.tx];
        if(entry===undefined)throw new Error(`no pinned prepared input for ${options.tx}`);
        const raw=await readFile(entry.receiptPath);
        if(createHash("sha256").update(raw).digest("hex")!==entry.receiptSha256)throw new Error("prepared input receipt hash differs");
        receiptPath=entry.receiptPath;
    }else receiptPath=(await context.fork.loadBoundary(context.original,{tx:options.tx})).receiptPath;
    const receipt=JSON.parse(await readFile(receiptPath,"utf8"));
    const bytes=await context.sdk.readArtifact(dirname(receiptPath),receipt.preparedRequest,256*1024*1024);
    const prepared=JSON.parse(bytes);
    const request=context.sdk.historicalRequest({requestId:`fork-check-${options.tx}`,family:prepared.family,genesisHash:context.genesisHash,
        candidate:prepared.candidate,runtimeBinding:prepared.fixture.runtime.binding,
        ...(options.replace===undefined?{}:{replacementTransactionBase64:options.replace.transactionBase64}),
        ...(options.overrides===undefined?{}:{requestedAccountOverrides:options.overrides}),metadataPolicy:prepared.metadataPolicy,
        limits:{timeoutMs:300000,maxOutputBytes:268435456,maxDiagnosticBytes:1048576}});
    let trace;
    if(options.collect!==undefined){
        if(options.collect.calls!==true||!Number.isSafeInteger(options.collect.maxEvents))
            throw new Error("cached-source drivers only request explicit bounded call journals");
        trace={require_complete:false,capture:{schema:"svm-capture-request/v2",executionMode:"jit",level:"calls",sbpfObservations:"none",
            filter:{programIds:[],instructionIndices:[]},limits:{maxBytes:268435456,maxEvents:options.collect.maxEvents,timeoutMs:180000}},
            bounds:{registerRows:0,memoryRows:null,maxInvocations:1024}};
    }
    const result=await context.offline.simulate({request,...(trace===undefined?{}:{trace}),signal:options.signal});
    return {...result,response:result.receipt.trace===undefined?undefined:{trace:result.receipt.trace}};
}
export function preparedClient(context){
    return {replay:options=>preparedJob(context,options),forkJobScope:()=>context.offline.forkJobScope(),projectBoundary:context.offline.projectBoundary.bind(context.offline)};
}
export async function readResult(context,options){
    const {receipt}=await preparedJob(context,options);
    if(receipt.outcome!=="COMPLETED")throw Object.assign(new Error(`SVM Replay could not run it: ${receipt.outcome}: ${receipt.error?.message}`),{details:{receipt}});
    const bytes=await context.sdk.readArtifact(dirname(receipt.receiptPath),receipt.output,256*1024*1024);
    return JSON.parse(bytes)[options.overrides===undefined?"replacement":"requestedOverrides"];
}
