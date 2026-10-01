import { requiredPath } from "../harness/paths";
/** Install exact retained captures and qualify one actual live CLI case per family. */
import {strict as assert} from "node:assert";
import {createHash} from "node:crypto";
import {constants} from "node:fs";
import {chmod,copyFile,mkdir,readFile,writeFile} from "node:fs/promises";
import {dirname,join,resolve} from "node:path";
import {spawn} from "node:child_process";
import {parse,stringify} from "lossless-json";
const [binaryArg,outArg,preparedArg,settledArg,selection="all"]=process.argv.slice(2);
if(!binaryArg||!outArg||!preparedArg)throw Error("usage: bun tests/debugging/debug_six.ts BINARY NEW_OUT PREPARED_47 [SETTLED_RUN [FAMILY,...]]");
const prototype=requiredPath("SVM_REPLAY_TEST_PROTOTYPE"),out=resolve(outArg),prepared=resolve(preparedArg);
const hash=(bytes:Uint8Array|string)=>createHash("sha256").update(bytes).digest("hex");
const save=(path:string,value:unknown)=>writeFile(path,JSON.stringify(value),{flag:"wx",mode:0o600});
const read=async(path:string)=>JSON.parse(await readFile(path,"utf8"));
const inputs=new Map<string,string>();
async function bound(path:string,expected?:string){const bytes=await readFile(path),pin=hash(bytes);
    if(expected!==undefined)assert.equal(pin,expected,path);inputs.set(path,pin);return bytes;}
async function install(source:string,dest:string,pin:string){await bound(source,pin);await copyFile(source,dest,constants.COPYFILE_EXCL);await chmod(dest,0o700);await bound(dest,pin);}
await mkdir(out,{mode:0o700});const binary=join(out,"svm-replay"),binaryPin=hash(await bound(resolve(binaryArg)));
await install(resolve(binaryArg),binary,binaryPin);inputs.delete(resolve(binaryArg));
const baselinePath=join(prepared,"catalog.json"),baselineSummary=await read(join(prepared,"summary.json"));
const baseline=JSON.parse((await bound(baselinePath,baselineSummary.catalogSha256)).toString());
const manifestPath=join(prototype,"poc/m13-tracing/workers/full-corpus/workers-manifest.json");
const manifest=JSON.parse((await bound(manifestPath)).toString());
const selected=manifest.workers.filter((row:any)=>selection==="all"||selection.split(",").includes(row.family));
assert.ok(selected.length>0);if(selection!=="all")assert.equal(selected.length,selection.split(",").length);
const retained:Record<string,string>={"v2-3":"v2-3-4","v3-0":"v3-0-4","v3-1":"v3-1-6","v4-0":"v4-0-3","v4-1":"v4-1-4"};
const summaries:any[]=[];
for(const row of selected){
    const familyDir=join(out,row.family);await mkdir(familyDir,{mode:0o700});
    const reference=baseline.workers.find((value:any)=>value.family===row.family);assert.ok(reference);assert.equal(reference.sha256,row.referenceSha256);
    const gate=JSON.parse((await bound(row.gate,row.gateSha256)).toString());assert.equal(gate.status,"PASS");
    let metadata:any,policy:any,bounds:any,buildPath:string,caseId:string,caseEvidence:string;
    if(row.family==="v4-2"){
        // Full-corpus manifest pins the later dd53… worker. The retained older
        // lifecycle test supplies the historical case and capture limits only.
        const sessionPath=join(prototype,"poc/m14-debugger/paused-accounts-results/final-6/session/session/manifest.json");
        const session=JSON.parse((await bound(sessionPath)).toString());metadata=session.worker;bounds=session.bounds;
        buildPath=join(prototype,"poc/m14-debugger/paused-accounts-results/final-6/worker-build.json");
        caseEvidence=join(prototype,"poc/m14-debugger/session-results/final-7/actual/debug-session/manifest.json");
        const lifecycle=JSON.parse((await bound(caseEvidence)).toString());policy=lifecycle.request;
        const oldFixture=join(dirname(caseEvidence),"fixture.json");
        const fixture=JSON.parse((await bound(oldFixture,lifecycle.files["fixture.json"].sha256)).toString());caseId=fixture.caseId;
    }else{
        assert.ok(retained[row.family]);caseEvidence=join(prototype,"poc/m14-debugger/legacy-session-results",retained[row.family],"options.json");
        const options=JSON.parse((await bound(caseEvidence)).toString());metadata=options.worker;policy=options.request;bounds=options.bounds;buildPath=options.buildReceiptPath;
        const fixture=JSON.parse((await bound(options.fixturePath)).toString());caseId=fixture.caseId;
    }
    assert.equal(metadata.sha256,row.binarySha256);assert.equal(metadata.family,row.family);assert.equal(metadata.executorSourceId,reference.executorSourceId);
    const buildBytes=await bound(buildPath,metadata.buildHash),build=JSON.parse(buildBytes.toString());assert.equal(build.binarySha256,row.binarySha256);assert.match(build.source.sha256,/^[a-f0-9]{64}$/);
    await install(join(dirname(baselinePath),reference.file),join(familyDir,"reference"),row.referenceSha256);
    await install(row.binary,join(familyDir,"capture"),row.binarySha256);
    const capture={...metadata,file:"capture",sourceSha256:build.source.sha256};
    const catalog={schema:baseline.schema,platform:baseline.platform,workers:[{...reference,file:"reference"}],captureWorkers:[{worker:capture,referenceSha256:row.referenceSha256,gateSha256:row.gateSha256}]};
    const catalogPath=join(familyDir,"catalog.json");await save(catalogPath,catalog);const catalogPin=hash(await bound(catalogPath));
    const originalPath=join(prepared,`${caseId}.json`),originalBytes=await bound(originalPath),request=JSON.parse(originalBytes.toString());
    assert.equal(request.candidate.id,caseId);assert.equal(request.candidate.executorSourceId,reference.executorSourceId);
    let requestBytes=originalBytes,settledProvenance:any;
    if(settledArg!==undefined){
        const stdoutPath=join(resolve(settledArg),`${caseId}.stdout.json`),settled=JSON.parse((await bound(stdoutPath)).toString());
        assert.equal(settled.outcome,"COMPLETED");assert.equal(settled.verification.status,"PASS");assert.equal(settled.verification.caseId,caseId);
        const receiptBytes=await bound(settled.receiptPath),stored=JSON.parse(receiptBytes.toString());
        const {receiptPath,...durable}=settled;assert.deepEqual(stored,durable);
        const job=dirname(receiptPath),fixturePath=join(job,stored.preparation.fixture.file);
        const fixture=await bound(fixturePath,stored.preparation.fixture.sha256);
        assert.equal(JSON.parse(fixture.toString()).caseId,caseId);
        const observations=join(job,stored.sourceObservations.file);await bound(observations,stored.sourceObservations.sha256);
        if(stored.preparedRequest!==undefined){
            requestBytes=await bound(join(job,stored.preparedRequest.file),stored.preparedRequest.sha256);
            const preparedRequest=JSON.parse(requestBytes.toString());assert.equal(preparedRequest.candidate.id,caseId);
            assert.deepEqual(preparedRequest.fixture,JSON.parse(fixture.toString()));
        }else{
            // Explicit offline prepare-then-debug: use the engine's hash-bound
            // settled fixture/evidence, never synthesize missing sysvar images.
            const updated:any=parse(originalBytes.toString());updated.fixture=parse(fixture.toString());
            updated.sourceEvidenceHashes=stored.preparation.sourceEvidenceHashes;
            const encoded=stringify(updated);assert.equal(typeof encoded,"string");requestBytes=Buffer.from(encoded!);
        }
        settledProvenance={receiptPath,receiptSha256:hash(receiptBytes),fixturePath,fixtureSha256:stored.preparation.fixture.sha256,
            sourceObservationsPath:observations,sourceObservationsSha256:stored.sourceObservations.sha256,
            requestMode:stored.preparedRequest===undefined?"explicit-settled-fixture-and-evidence":"exact-saved-prepared-request"};
    }
    // Change only this explicit transport bound, preserving all fixture bytes.
    const needle=`"maxOutputBytes":${request.limits.maxOutputBytes}`;
    const text=requestBytes.toString();assert.equal(text.split(needle).length,2);
    const requestPath=join(familyDir,"request.json");await writeFile(requestPath,text.replace(needle,`"maxOutputBytes":${policy.limits.maxBytes}`),{flag:"wx",mode:0o600});
    const identityPath=join(familyDir,"identity.json");await save(identityPath,{catalogPath,catalogSha256:catalogPin,requestPath,policy,bounds,reference:catalog.workers[0],capture,gateSha256:row.gateSha256,
        provenance:{manifestPath,manifestSha256:inputs.get(manifestPath),caseEvidence,caseEvidenceSha256:inputs.get(caseEvidence),originalRequestPath:originalPath,originalRequestSha256:hash(originalBytes),buildPath,buildSha256:metadata.buildHash,settled:settledProvenance}});
    console.log(JSON.stringify({family:row.family,caseId,phase:"start",maxBytes:policy.limits.maxBytes,timeoutMs:policy.limits.timeoutMs}));
    const child=spawn(process.execPath,[join(import.meta.dir,"debug_cli.ts"),binary,join(familyDir,"run"),identityPath,"step"],{stdio:"inherit"});
    const code=await new Promise<number|null>((done,reject)=>{child.once("error",reject);child.once("close",done);});
    const summary=await read(join(familyDir,"run/summary.json"));summaries.push({family:row.family,...summary});
    await save(join(familyDir,"result.json"),{code,summary});
    // A real gap remains recorded and stops this tranche; never select an easier
    // replacement fixture or rewrite a retained expected output.
    if(code!==0)break;
}
for(const [path,pin] of inputs)await bound(path,pin);
const summary={schema:"svm-replay-m17-debug-six/v1",selectedFamilies:selected.map((row:any)=>row.family),status:summaries.length===selected.length&&summaries.every(row=>row.status==="PASS")?"PASS":"FAIL",binarySha256:binaryPin,
    manifestSha256:inputs.get(manifestPath),cases:summaries,inputPins:Object.fromEntries(inputs)};
await save(join(out,"summary.json"),summary);console.log(JSON.stringify({status:summary.status,cases:summaries.map(row=>({family:row.family,caseId:row.caseId,status:row.status,elapsedMs:row.elapsedMs}))}));
if(summary.status!=="PASS")process.exitCode=1;
