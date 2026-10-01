import { requiredPath } from "../harness/paths";
/** Native JIT capture over the unchanged finite corpus, with explicit settled inputs. */
import {strict as assert} from "node:assert";
import {createHash} from "node:crypto";
import {constants} from "node:fs";
import {chmod,copyFile,mkdir,readFile,writeFile} from "node:fs/promises";
import {dirname,join,resolve} from "node:path";
import {spawn} from "node:child_process";
import {parse,stringify} from "lossless-json";
import {frozenPolicy} from "../harness/frozen_policy";
const [binaryArg,outArg,preparedArg,settledArg,debugFirstArg,debugSettledArg,selection="all",catalogRootArg,priorRootArg,expectedRequestArg]=process.argv.slice(2);
if(!binaryArg||!outArg||!preparedArg||!settledArg||!debugFirstArg||!debugSettledArg)throw Error("usage: bun tests/tracing/trace_corpus.ts BINARY NEW_OUT PREPARED_47 SETTLED_RUN DEBUG_FIRST DEBUG_SETTLED [CASE|all]");
const out=resolve(outArg),prepared=resolve(preparedArg),settled=resolve(settledArg);
const isolated=catalogRootArg!==undefined&&catalogRootArg!=="-";
const priorRoots=priorRootArg===undefined||priorRootArg==="-"?[]:priorRootArg.split(",").map(path=>resolve(path));
if(expectedRequestArg!==undefined)assert.notEqual(selection,"all","request-byte comparison requires one selected case");
const hash=(bytes:Uint8Array|string)=>createHash("sha256").update(bytes).digest("hex");
const save=(path:string,value:unknown)=>writeFile(path,JSON.stringify(value),{flag:"wx",mode:0o600});
const inputs=new Map<string,string>();
async function bound(path:string,expected?:string){const bytes=await readFile(path),pin=hash(bytes);if(expected!==undefined)assert.equal(pin,expected,path);inputs.set(path,pin);return bytes;}
async function install(source:string,dest:string,pin:string){await bound(source,pin);await copyFile(source,dest,constants.COPYFILE_EXCL);await chmod(dest,0o700);await bound(dest,pin);}
await mkdir(out,{mode:0o700});const binary=isolated?resolve(binaryArg):join(out,"svm-replay"),binaryPin=hash(await bound(resolve(binaryArg)));
if(!isolated){await install(resolve(binaryArg),binary,binaryPin);inputs.delete(resolve(binaryArg));}
const prototype=requiredPath("SVM_REPLAY_TEST_PROTOTYPE");
const manifestPath=join(prototype,"poc/m13-tracing/workers/full-corpus/workers-manifest.json"),manifest=JSON.parse((await bound(manifestPath)).toString());
const workers:any[]=[],captureWorkers:any[]=[];let platform:any;
if(!isolated)await mkdir(join(out,"workers"),{mode:0o700});
for(const row of isolated?[]:manifest.workers){
    const source=resolve(["v2-3","v3-0","v3-1"].includes(row.family)?debugFirstArg:debugSettledArg,row.family);
    const identity=JSON.parse((await bound(join(source,"identity.json"))).toString());
    const catalog=JSON.parse((await bound(identity.catalogPath,identity.catalogSha256)).toString());
    assert.equal(catalog.workers.length,1);assert.equal(catalog.captureWorkers.length,1);
    const reference=catalog.workers[0],capture=catalog.captureWorkers[0];
    assert.equal(reference.family,row.family);assert.equal(reference.sha256,row.referenceSha256);assert.equal(capture.worker.sha256,row.binarySha256);
    assert.equal(capture.gateSha256,row.gateSha256);assert.equal(capture.referenceSha256,row.referenceSha256);await bound(row.gate,row.gateSha256);
    if(platform!==undefined)assert.deepEqual(platform,catalog.platform);platform=catalog.platform;
    const dir=join(out,"workers",row.family);await mkdir(dir,{mode:0o700});
    await install(join(dirname(identity.catalogPath),reference.file),join(dir,"reference"),reference.sha256);
    await install(join(dirname(identity.catalogPath),capture.worker.file),join(dir,"capture"),capture.worker.sha256);
    workers.push({...reference,file:`workers/${row.family}/reference`});captureWorkers.push({...capture,worker:{...capture.worker,file:`workers/${row.family}/capture`}});
}
const catalogPath=join(isolated?resolve(catalogRootArg!):out,"catalog.json");
if(!isolated)await save(catalogPath,{schema:"svm-replay-workers/v1",platform,workers,captureWorkers});
const catalogPin=hash(await bound(catalogPath));
if(isolated){const pins=JSON.parse((await bound(join(resolve(catalogRootArg!),"installation.json"))).toString());assert.equal(pins.binarySha256,binaryPin);assert.equal(pins.catalogSha256,catalogPin);}
else await save(join(out,"installation.json"),{binarySha256:binaryPin,catalogSha256:catalogPin});
const corpusPath=join(requiredPath("SVM_REPLAY_TEST_ARTIFACTS"), "m12-sealed-historical-XmOnk2/historical/regression-inputs.json");
const corpus=JSON.parse((await bound(corpusPath)).toString());
const cases=corpus.cases.filter((row:any)=>selection==="all"||row.candidate.id===selection);assert.ok(cases.length>0);if(selection==="all")assert.equal(cases.length,47);
// Preserved M13 full-corpus export policy. Producer rows remain explicitly bounded;
// a completed historical replay does not imply an untruncated instruction trace.
const trace={capture:{schema:"svm-capture-request/v2",executionMode:"jit",level:"sbpf",sbpfObservations:"pc-registers",filter:{programIds:[],instructionIndices:[]},
    limits:{maxBytes:256*1024*1024,maxEvents:1000000,timeoutMs:180000}},bounds:{registerRows:4,memoryRows:null,maxInvocations:128}};
await bound(join(prototype,"poc/m13-tracing/workers/full-corpus/case.ts"));await save(join(out,"trace.json"),trace);
async function prepareCase(row:any,dir:string,policy:any){
    // The lossless number graphs can be much larger than their serialized JSON.
    // Keep them out of the native execution lifetime without changing parsing.
    const id=row.candidate.id,originalPath=join(prepared,`${id}.json`),original=await bound(originalPath),request:any=parse(original.toString());
    request.metadataPolicy=policy.metadataPolicy;
    const prior=JSON.parse((await bound(join(settled,`${id}.stdout.json`))).toString());assert.equal(prior.outcome,"COMPLETED");assert.equal(prior.verification.status,"PASS");
    const {receiptPath,...durable}=prior,storedBytes=await bound(receiptPath),stored=JSON.parse(storedBytes.toString());assert.deepEqual(stored,durable);
    const fixturePath=join(dirname(receiptPath),stored.preparation.fixture.file),fixture=await bound(fixturePath,stored.preparation.fixture.sha256);
    assert.equal(JSON.parse(fixture.toString()).caseId,id);await bound(join(dirname(receiptPath),stored.sourceObservations.file),stored.sourceObservations.sha256);
    request.fixture=parse(fixture.toString());request.sourceEvidenceHashes=stored.preparation.sourceEvidenceHashes;
    assert.ok(request.sourceEvidenceHashes.includes(policy.evidenceSha256),"settled evidence lacks frozen admission receipt");
    request.limits.maxOutputBytes=trace.capture.limits.maxBytes;
    const encoded=stringify(request);assert.equal(typeof encoded,"string");await writeFile(join(dir,"request.json"),encoded!,{flag:"wx",mode:0o600});
    await save(join(dir,"provenance.json"),{originalPath,originalSha256:hash(original),receiptPath,receiptSha256:hash(storedBytes),fixturePath,fixtureSha256:hash(fixture),sourceEvidenceHashes:stored.preparation.sourceEvidenceHashes,requestMode:"explicit-settled-fixture-and-evidence"});
    return {family:request.family,timeoutMs:Number(request.limits.timeoutMs),requestSha256:hash(encoded!)};
}
function stateMatches(actual:any,expected:any,key:string){
    assert.ok(expected.presence==="present"||expected.presence==="absent",key);
    if(expected.presence==="absent"){assert.ok(actual===null||String(actual.lamports)==="0",key);return;}
    assert.ok(actual,key);assert.equal(String(actual.lamports),expected.lamports,key);assert.equal(actual.owner,expected.owner,key);
    assert.equal(actual.executable,expected.executable,key);assert.equal(String(actual.rent_epoch),expected.rentEpoch,key);
    assert.ok(Array.isArray(actual.data)&&actual.data.every((byte:any)=>Number.isInteger(byte)&&byte>=0&&byte<=255),key);
    assert.equal(hash(Buffer.from(actual.data)),expected.dataHash,key);if(expected.dataLength!==undefined)assert.equal(actual.data.length,Number(expected.dataLength),key);
}
function boundaryProof(events:any[],output:any){
    const scopes=events.filter(row=>row.kind==="journal_scope");assert.equal(scopes.length,1);
    const journal=events.filter(row=>row.kind==="journal_event");assert.ok(journal.every(row=>row.journalIndex===scopes[0].journalIndex));
    const begins=journal.filter(row=>row.event.kind==="transaction_begin"),commits=journal.filter(row=>row.event.kind==="transaction_commit");
    assert.equal(begins.length,1);assert.equal(commits.length,1,"full-corpus policy did not retain committed boundary");
    const before=new Map(begins[0].event.accounts.map((row:any)=>[row.pubkey,row.state]));const after=new Map(commits[0].event.accounts.map((row:any)=>[row.pubkey,row.state]));
    let beforeCount=0,afterCount=0;assert.ok(output.original.accountTransitions.length>0);
    for(const row of output.original.accountTransitions){if(before.has(row.pubkey)){stateMatches(before.get(row.pubkey),row.before,row.pubkey);beforeCount++;}
        assert.ok(after.has(row.pubkey),row.pubkey);stateMatches(after.get(row.pubkey),row.after,row.pubkey);afterCount++;}
    assert.ok(beforeCount>0);assert.ok(["ok","err"].includes(output.original.status));
    assert.equal(commits[0].event.outcome.status,output.original.status==="ok"?"success":"failure");
    const executions=events.filter(row=>row.kind==="execution_scope");assert.equal(executions.length,1);assert.equal(executions[0].executionMode,"jit");
    const registers=events.filter(row=>row.kind==="registers");assert.ok(registers.length>0&&registers.length<=4);
    return {beforeAccounts:beforeCount,committedAccounts:afterCount,registerRows:registers.length};
}
const results:any[]=[],started=performance.now();
for(const row of cases){
    const id=row.candidate.id,dir=isolated?out:join(out,id);
    if(selection==="all"){
        // Each parser/validator gets a fresh process. Large JSON account arrays
        // must not accumulate in a single long-lived JS heap under the 8 GiB cap.
        Bun.gc(true);
        const args=[import.meta.path,binary,dir,prepared,settled,debugFirstArg,debugSettledArg,id,out];if(priorRootArg!==undefined)args.push(priorRootArg);
        const child=spawn(process.execPath,args,{stdio:"inherit"});const code=await new Promise<number|null>((done,reject)=>{child.once("error",reject);child.once("close",done);});
        let childSummary:any;
        try{childSummary=JSON.parse(await readFile(join(dir,"summary.json"),"utf8"));}
        catch(error:any){if(error.code!=="ENOENT")throw error;console.error(`${id}: isolated validator exited ${code} before a summary`);results.push({caseId:id,status:"FAIL",code,error:"isolated validator did not finalize"});break;}
        assert.equal(childSummary.results.length,1);assert.equal(childSummary.results[0].caseId,id);assert.equal(childSummary.binarySha256,binaryPin);assert.equal(childSummary.catalogSha256,catalogPin);
        results.push(childSummary.results[0]);for(const [path,pin] of Object.entries(childSummary.inputPins)){if(inputs.has(path))assert.equal(inputs.get(path),pin);inputs.set(path,pin as string);}
        if(code!==0||childSummary.status!=="PASS")break;continue;
    }
    if(!isolated)await mkdir(dir,{mode:0o700});
    const policy=await frozenPolicy(row);await bound(row.frozenReceiptPath,policy.evidenceSha256);
    const request=await prepareCase(row,dir,policy);
    if(expectedRequestArg!==undefined){const source=resolve(expectedRequestArg);await bound(source,request.requestSha256);
        await save(join(dir,"request-byte-comparison.json"),{source,sha256:request.requestSha256,status:"IDENTICAL"});}
    let reusedFrom:string|undefined;
    for(const priorRoot of priorRoots){const path=join(priorRoot,id,"summary.json");let previous:any;
        try{previous=JSON.parse((await bound(path)).toString());}catch(error:any){if(error.code!=="ENOENT")throw error;console.error(`${id}: no finalized prior summary at ${path}`);}
        if(previous!==undefined){assert.equal(previous.status,"PASS");if(previous.schema==="svm-replay-m17-trace-corpus/v1"){assert.equal(previous.results.length,1);previous=previous.results[0];}
            assert.equal(previous.status,"PASS");assert.equal(previous.caseId,id);reusedFrom=dirname(path);assert.equal(hash(await bound(join(reusedFrom,"request.json"))),request.requestSha256);break;}}
    Bun.gc(true);
    console.log(JSON.stringify({caseId:id,phase:reusedFrom===undefined?"start":"revalidate-preserved",family:request.family,metadataPolicy:policy.metadataPolicy}));
    const start=performance.now(),stdout:Buffer[]=[],stderr:Buffer[]=[];let code:number|null;
    if(reusedFrom!==undefined){stdout.push(await bound(join(reusedFrom,"stdout.json")));stderr.push(await bound(join(reusedFrom,"stderr.txt")));code=0;}
    else{const child=spawn(binary,["simulate","--request",join(dir,"request.json"),"--catalog",catalogPath,"--catalog-sha256",catalogPin,"--trace",join(out,"trace.json"),"--data-dir",join(dir,"data")],{stdio:["ignore","pipe","pipe"]});
        child.stdout.on("data",chunk=>stdout.push(chunk));child.stderr.on("data",chunk=>stderr.push(chunk));
        const timeout=setTimeout(()=>{console.error(`${id}: test deadline, cancelling CLI`);child.kill("SIGTERM");},request.timeoutMs+15000);
        code=await new Promise<number|null>((done,reject)=>{child.once("error",reject);child.once("close",done);}).finally(()=>clearTimeout(timeout));}
    await writeFile(join(dir,"stdout.json"),Buffer.concat(stdout),{flag:"wx",mode:0o600});await writeFile(join(dir,"stderr.txt"),Buffer.concat(stderr),{flag:"wx",mode:0o600});
    let receipt:any,error:unknown;const exports:any[]=[];
    try{
        receipt=JSON.parse(Buffer.concat(stdout).toString());assert.equal(code,0,JSON.stringify(receipt.error));assert.equal(receipt.outcome,"COMPLETED");
        assert.equal(receipt.currentStateFallback,false);assert.equal(receipt.reconstructedFromSources,false);
        assert.equal(receipt.controlVerification.status,"PASS");assert.equal(receipt.verification.status,"PASS");
        const {receiptPath,...durable}=receipt;assert.deepEqual(JSON.parse(await readFile(receiptPath,"utf8")),durable);
        const execution=receipt.trace.execution;assert.equal(execution.status,"PASS");assert.equal(execution.executionMode,"jit");assert.equal(execution.workerCalls,3);
        assert.equal(execution.implementationSha256,binaryPin);assert.equal(receipt.catalogSha256,catalogPin);
        assert.deepEqual(execution.events.map((event:any)=>event.mode),["reference","off","capture"]);assert.equal(execution.events[0].responseSha256,execution.events[1].responseSha256);
        const outputBytes=await readFile(join(dirname(receiptPath),receipt.output.file));assert.equal(hash(outputBytes),receipt.output.sha256);const output=JSON.parse(outputBytes.toString());
        assert.equal(receipt.trace.exports.length,1);
        for(const item of receipt.trace.exports){const payload=await readFile(join(dirname(receiptPath),item.file));assert.equal(hash(payload),item.sha256);assert.equal(item.sha256,item.artifact.payload.sha256);
            assert.equal(payload.length,item.artifact.payload.bytes);const events=JSON.parse(payload.toString());assert.equal(events.length,item.artifact.payload.events);
            assert.ok(["COMPLETE","TRUNCATED"].includes(item.artifact.status));assert.equal(item.artifact.executionMode,"jit");
            exports.push({phase:item.phase,status:item.artifact.status,bytes:payload.length,events:events.length,rawBoundaryParity:boundaryProof(events,output)});}
    }catch(cause){error=String(cause);}
    const result={caseId:id,family:request.family,status:error===undefined?"PASS":"FAIL",code,outcome:receipt?.outcome,error,engineError:receipt?.error,metadataPolicy:policy.metadataPolicy,
        receiptPath:receipt?.receiptPath,elapsedMs:performance.now()-start,exports,networkRequests:0,reusedFrom};await save(join(dir,isolated?"case-summary.json":"summary.json"),result);results.push(result);console.log(JSON.stringify(result));
    if(error!==undefined)break;
}
for(const [path,pin] of inputs){await bound(path,pin);Bun.gc(true);}
const summary={schema:"svm-replay-m17-trace-corpus/v1",selection,status:results.length===cases.length&&results.every(row=>row.status==="PASS")?"PASS":"FAIL",elapsedMs:performance.now()-started,
    requestedCases:cases.length,results,binarySha256:binaryPin,catalogPath,catalogSha256:catalogPin,inputPins:Object.fromEntries(inputs),networkRequests:0};
await save(join(out,"summary.json"),summary);console.log(JSON.stringify({status:summary.status,selection,cases:results.length,elapsedMs:summary.elapsedMs}));if(summary.status!=="PASS")process.exitCode=1;
