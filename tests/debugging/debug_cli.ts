/** Real pinned-worker CLI debug protocol. Historical outputs are never command inputs. */
import {strict as assert} from "node:assert";
import {createHash} from "node:crypto";
import {constants} from "node:fs";
import {chmod,copyFile,mkdir,readFile,readdir,writeFile} from "node:fs/promises";
import {dirname,join,resolve} from "node:path";
import {spawn} from "node:child_process";
const [binaryArg,outArg,identityArg,mode="step"]=process.argv.slice(2);
if(!binaryArg||!outArg||!identityArg||!["step","accounts","eof","sigterm","malformed"].includes(mode))throw new Error("usage: bun tests/debugging/debug_cli.ts BINARY NEW_OUT PINNED_IDENTITY [step|accounts|eof|sigterm|malformed]");
const digest=(bytes:Uint8Array|string)=>createHash("sha256").update(bytes).digest("hex");
const save=(path:string,value:unknown)=>writeFile(path,JSON.stringify(value),{flag:"wx",mode:0o600});
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const identityBytes=await readFile(resolve(identityArg)),identity=JSON.parse(identityBytes.toString());
const catalogBytes=await readFile(identity.catalogPath);assert.equal(digest(catalogBytes),identity.catalogSha256);
const requestBytes=await readFile(identity.requestPath),request=JSON.parse(requestBytes.toString());
const binaryBytes=await readFile(resolve(binaryArg)),binary=join(out,"svm-replay");
await copyFile(resolve(binaryArg),binary,constants.COPYFILE_EXCL);await chmod(binary,0o700);
assert.equal(digest(await readFile(binary)),digest(binaryBytes));
await save(join(out,"trace.json"),{capture:identity.policy,bounds:identity.bounds});
const knownPids=new Set<number>();
async function descendants(pid:number):Promise<void>{
    // Linux records a child against its spawning thread, not every process task.
    for(const tid of await readdir(`/proc/${pid}/task`)){
        let children:string;
        try{children=await readFile(`/proc/${pid}/task/${tid}/children`,"utf8");}
        catch(error:any){if(error.code==="ENOENT"){console.error(`ownership observer: thread ${pid}/${tid} exited`);continue;}throw error;}
        for(const child of children.trim().split(/\s+/).filter(Boolean).map(Number)){
            assert.ok(Number.isSafeInteger(child)&&child>0);if(knownPids.has(child))continue;
            knownPids.add(child);await descendants(child);
        }
    }
}
const start=performance.now();
const child=spawn(binary,["simulate","--debug","--request",identity.requestPath,"--catalog",identity.catalogPath,"--catalog-sha256",identity.catalogSha256,
    "--trace",join(out,"trace.json"),"--data-dir",join(out,"data")],{cwd:out,stdio:["pipe","pipe","pipe"]});
const stdout:Buffer[]=[],stderr:Buffer[]=[],events:any[]=[],commands:any[]=[];
let pending="",chain=Promise.resolve(),interactionError:unknown,receipt:any;
let phase="original-control",selected:any,before:any,after:any,requestId=0,acted=false;
let accountInspection:any,accountDescriptor:any,accountEntry:any;
function fail(error:unknown){if(interactionError===undefined)interactionError=error;child.kill("SIGTERM");}
function send(target:any,action:any){const command={requestId:++requestId,phase,target,action};commands.push(command);child.stdin.write(JSON.stringify(command)+"\n");}
async function event(value:any){
    if(value.schema==="svm-replay-receipt/v1"||value.schema==="svm-replay-cli-error/v1"){assert.equal(receipt,undefined,"duplicate final receipt");receipt=value;return;}
    assert.equal(receipt,undefined,"event after final receipt");events.push(value);
    if(value.kind==="phase-start")phase=value.phase;
    if(value.kind==="entry"&&value.state==="STOPPED"){
        if(!acted){
            assert.ok(child.pid);await descendants(child.pid);assert.ok(knownPids.size>0,"live worker ownership evidence missing");acted=true;
            if(mode==="eof"){child.stdin.end();return;}
            if(mode==="sigterm"){child.kill("SIGTERM");return;}
            if(mode==="malformed"){child.stdin.write('{"requestId":1,"requestId":2}\n');return;}
            selected=value.target;accountEntry=value;send(selected,{kind:mode==="accounts"?"metadata":"registers"});return;
        }
        if(mode==="step"||mode==="accounts")send(value.target,{kind:"continue"});
    }
    if(mode!=="step"&&mode!=="accounts")return;
    if(mode==="accounts"&&value.kind==="response"){
        if(value.result?.fields){
            if(value.result.fields.account_layout!=="serialized-addresses-v1"){
                accountInspection={status:"UNAVAILABLE",reason:"runtime does not advertise serialized-addresses-v1"};send(selected,{kind:"registers"});
            }else send(selected,{kind:"account-directory"});
            return;
        }
        if(value.result?.accounts){
            assert.ok(value.result.accounts.length>0,"advertised directory has no named account");
            accountDescriptor=value.result.accounts[0];assert.equal(typeof accountDescriptor.pubkey,"string");
            send(selected,{kind:"inspect-account",pubkey:accountDescriptor.pubkey,max_data_bytes:1});return;
        }
        if(value.result?.scope==="paused-vm-serialized-working-account"){
            assert.equal(value.result.pubkey,accountDescriptor.pubkey);assert.equal(value.result.program,accountEntry.identity.program);
            assert.equal(value.result.invocationIndex,selected.invocationIndex);assert.match(value.result.lamports,/^(0|[1-9][0-9]*)$/);
            assert.ok(BigInt(value.result.lamports)<=18446744073709551615n);assert.ok(value.result.dataHex.length<=2);
            assert.match(value.result.runtimeAccountState,/unavailable/);assert.match(value.result.committedState,/unavailable while paused/);
            accountInspection={status:"PASS",descriptor:accountDescriptor,observed:value.result};send(selected,{kind:"registers"});return;
        }
    }
    if(value.kind==="response"&&value.result?.registers){
        if(before===undefined){before=value.result.registers;send(selected,{kind:"step-over"});}
        else {after=value.result.registers;send(null,{kind:"continue-all"});}
    }
    if(value.kind==="stop"&&value.stop.kind==="stopped"){
        const token=value.target;
        if(selected&&selected.executionIndex===token.executionIndex&&selected.invocationIndex===token.invocationIndex&&after===undefined){selected=token;send(token,{kind:"registers"});}
        else send(token,{kind:"continue"});
    }
}
child.stdin.on("error",fail);
child.stdout.on("data",chunk=>{
    stdout.push(chunk);pending+=chunk.toString();
    if(pending.length>17*1024*1024){fail(new Error("CLI JSONL line exceeds test bound"));return;}
    while(pending.includes("\n")){const index=pending.indexOf("\n"),line=pending.slice(0,index);pending=pending.slice(index+1);
        chain=chain.then(()=>event(JSON.parse(line))).catch(fail);
    }
});
child.stderr.on("data",chunk=>stderr.push(chunk));
const timeout=setTimeout(()=>fail(new Error("debug CLI test deadline")),request.limits.timeoutMs+15000);
const code=await new Promise<number|null>((done,fail)=>{child.once("error",fail);child.once("close",done);}).finally(()=>clearTimeout(timeout));
await chain;assert.equal(pending,"");
await writeFile(join(out,"stdout.jsonl"),Buffer.concat(stdout),{flag:"wx",mode:0o600});
await writeFile(join(out,"stderr.txt"),Buffer.concat(stderr),{flag:"wx",mode:0o600});
await save(join(out,"commands.json"),commands);await save(join(out,"observed-events.json"),events);
const unreaped:number[]=[];
for(const pid of knownPids){try{process.kill(pid,0);unreaped.push(pid);}catch(error:any){if(error.code!=="ESRCH")throw error;}}
const checks:any={stdoutFraming:true,workerTreeReaped:unreaped.length===0,originalInputUnchanged:digest(await readFile(identity.requestPath))===digest(requestBytes),catalogUnchanged:digest(await readFile(identity.catalogPath))===identity.catalogSha256};
let validationError:unknown;
try{
    assert.equal(interactionError,undefined);assert.ok(acted,"worker never reached a live pause");assert.deepEqual(unreaped,[]);
    assert.ok(checks.originalInputUnchanged&&checks.catalogUnchanged);assert.ok(receipt,"final receipt missing");
    if(mode==="step"||mode==="accounts"){
        assert.equal(code,0,JSON.stringify({outcome:receipt.outcome,errorCode:receipt.error?.code,errorMessage:receipt.error?.message}));assert.equal(receipt.outcome,"COMPLETED");
        assert.equal(receipt.controlVerification.status,"PASS");assert.equal(receipt.verification.status,"PASS");
        const {receiptPath,...durable}=receipt;assert.deepEqual(JSON.parse(await readFile(receiptPath,"utf8")),durable);
        assert.ok(before&&after);assert.notEqual(before[11],after[11],"real PC did not advance");
        if(mode==="accounts"){assert.ok(accountInspection);if(accountInspection.status==="PASS")assert.equal(accountInspection.observed.pc,before[11]);}
        const storedEvents=await readFile(join(dirname(receipt.receiptPath),receipt.debug.events.file));
        assert.equal(digest(storedEvents),receipt.debug.events.sha256);
        // Engine persists invocation/session events; finalization notifications
        // are emitted separately after their finalized metadata has been checked.
        assert.deepEqual(JSON.parse(storedEvents.toString()),events.filter(row=>!["transaction-complete","session-complete"].includes(row.kind)),"durable events differ from live JSONL");
        const completed=events.filter(row=>row.kind==="session-complete");assert.equal(completed.length,1);
        assert.deepEqual(completed[0].receipt,receipt.debug.execution);
        assert.equal(events.filter(row=>row.kind==="transaction-complete").length,receipt.debug.exports.length);
        assert.ok(receipt.debug.exports.length>0,"missing live capture export");
        for(const item of receipt.debug.exports){const bytes=await readFile(join(dirname(receipt.receiptPath),item.file));
            assert.equal(digest(bytes),item.sha256);assert.equal(item.sha256,item.artifact.payload.sha256);
            assert.equal(bytes.length,item.artifact.payload.bytes);assert.equal(JSON.parse(bytes.toString()).length,item.artifact.payload.events);}
        const output=await readFile(join(dirname(receipt.receiptPath),receipt.output.file));assert.equal(digest(output),receipt.output.sha256);
        checks.pcAdvanced=true;checks.eventsAndExportsHashBound=true;
    }else{
        assert.notEqual(code,0);assert.equal(receipt.outcome,mode==="malformed"?"ERROR":"CANCELLED",JSON.stringify(receipt));
        assert.equal(receipt.error.code,mode==="malformed"?"DEBUG_INPUT":"WORKER_CANCELLED");
        const path=mode==="malformed"?receipt.error.details.engineReceiptPath:receipt.receiptPath;
        const stored=JSON.parse(await readFile(path,"utf8"));assert.equal(stored.outcome,"CANCELLED");assert.equal(stored.error.code,"WORKER_CANCELLED");
        checks.durableCancellation=true;
    }
}catch(error){validationError=String(error);}
const summary={schema:"svm-replay-m17-debug-cli-test/v1",status:validationError===undefined?"PASS":"FAIL",mode,code,caseId:request.candidate.id,
    error:validationError,interactionError:interactionError===undefined?undefined:String(interactionError),outcome:receipt?.outcome,
    receiptPath:receipt?.receiptPath,engineReceiptPath:receipt?.error?.details?.engineReceiptPath,
    elapsedMs:performance.now()-start,eventCount:events.length,commandCount:commands.length,
    captureArtifacts:receipt?.debug?.exports?.map((item:any)=>({phase:item.phase,status:item.artifact.status,bytes:item.artifact.payload.bytes,events:item.artifact.payload.events})),
    accountInspection,
    configuredBudgets:{requestTimeoutMs:request.limits.timeoutMs,captureTimeoutMs:identity.policy.limits.timeoutMs},
    knownPids:[...knownPids],unreaped,checks,binarySha256:digest(binaryBytes),identitySha256:digest(identityBytes),requestSha256:digest(requestBytes),catalogSha256:identity.catalogSha256};
await save(join(out,"summary.json"),summary);console.log(JSON.stringify(summary));
if(summary.status!=="PASS")process.exitCode=1;
