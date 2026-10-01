/** Real consumers and pinned historical source. No current-state provider or wallet. */
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {createRequire} from "node:module";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {pathToFileURL} from "node:url";
import {connect} from "node:net";
import {offlineTransfer} from "../../../examples/anchor/transfer.mjs";
const require=createRequire(new URL("../../../examples/package.json",import.meta.url));
const {Surfnet}=require("@solana/surfpool");
const {VersionedTransaction,SystemProgram}=require("@solana/web3.js");
const [entry,bundle,pin,sourceRun,captures,outArg]=process.argv.slice(2);
if(!outArg)throw new Error("usage: node integrations.mjs INSTALLED_SDK_ENTRY BUNDLE PIN SOURCE_RUN CAPTURES NEW_OUT");
const {Replay,readArtifact}=await import(pathToFileURL(resolve(entry)));
const {withHistoricalReplay,anchorRequest,assertCompleted}=await import(pathToFileURL(join(dirname(resolve(entry)),"adapters/index.js")));
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const save=(name,v)=>writeFile(join(out,name),JSON.stringify(v),{flag:"wx",mode:0o600});
const hash=b=>createHash("sha256").update(b).digest("hex");
const request=JSON.parse(await readFile(join(sourceRun,"m7-profile-03.request.json"),"utf8"));
const sourcePath=join(captures,request.candidate.id,"captured/manifest.json");
const sourceBytes=await readFile(sourcePath),source=JSON.parse(sourceBytes);
const captured=source.entries.filter(e=>e.query.kind==="transaction"&&e.query.signature===request.candidate.targetSignature);
assert.equal(captured.length,1);
const wireBytes=await readFile(join(dirname(sourcePath),captured[0].file));assert.equal(hash(wireBytes),captured[0].sha256);
const wire=JSON.parse(wireBytes).value,transaction=VersionedTransaction.deserialize(Buffer.from(wire,"base64"));
const message=transaction.message,keys=message.staticAccountKeys;
const last=message.compiledInstructions.at(-1);
assert.ok(keys[last.programIdIndex].equals(SystemProgram.programId));
assert.equal(Buffer.from(last.data).readUInt32LE(),2);
const payer=keys[last.accountKeyIndexes[0]].toBase58(),recipient=keys[last.accountKeyIndexes[1]].toBase58();
assert.equal(payer,keys[0].toBase58());
const replay=await Replay.open({bundlePath:resolve(bundle),bundleSha256:pin,dataDir:join(out,"data"),cache:"off",
    source:{path:sourcePath,sha256:hash(sourceBytes)}});
const start=performance.now(),local=Surfnet.startWithConfig({offline:true,blockProductionMode:"manual",airdropSol:0});
const rpcUrl=local.rpcUrl;let surfResult;
try{
    const companion=withHistoricalReplay(local,replay);
    assert.equal(companion.rpcUrl,rpcUrl);
    surfResult=assertCompleted(await companion.simulateTransactionAt({request}));
    assert.equal(Object.hasOwn(local,"simulateTransactionAt"),false);
}finally{local.stop();}
const endpoint=new URL(rpcUrl);
const closed=await new Promise((done,fail)=>{const socket=connect({host:endpoint.hostname,port:Number(endpoint.port)});
    socket.setTimeout(2000);socket.once("connect",()=>{socket.destroy();done(false);});
    socket.once("error",e=>{if(e.code==="ECONNREFUSED")done(true);else fail(e);});
    socket.once("timeout",()=>{socket.destroy();fail(new Error("owned Surfpool shutdown probe timed out"));});});
assert.equal(closed,true,"owned Surfpool listener must stop");
const {transaction:replacement,rpcCalls}=await offlineTransfer({payer,recipient,blockhash:message.recentBlockhash,lamports:"1"});
assert.equal(rpcCalls,0);
const anchorInput=anchorRequest({...request,requestId:"m18-anchor-transfer"},replacement);
const anchorResult=assertCompleted(await replay.simulate({request:anchorInput}));
const payload=JSON.parse((await readArtifact(dirname(anchorResult.receipt.receiptPath),anchorResult.receipt.output,64*1024*1024)).toString());
const original=JSON.parse((await readArtifact(dirname(surfResult.receipt.receiptPath),surfResult.receipt.output,64*1024*1024)).toString());
assert.deepEqual(payload.original,original.original);assert.deepEqual(payload.prefix,original.prefix);
assert.equal(payload.replacement.status,"ok");
const transition=payload.replacement.accountTransitions.find(t=>t.pubkey===recipient);assert.ok(transition);
assert.equal(BigInt(transition.after.lamports)-BigInt(transition.before.lamports),1n);
const summary={status:"PASS",caseId:request.candidate.id,bundleSha256:pin,sourceSha256:hash(sourceBytes),
    surfpool:{version:require("@solana/surfpool/package.json").version,receiptPath:surfResult.receipt.receiptPath,ownedListenerStopped:closed},
    anchor:{offlineRpcCalls:rpcCalls,receiptPath:anchorResult.receipt.receiptPath,payer,recipient,lamportDelta:"1",originalControlUnchanged:true},
    elapsedMs:performance.now()-start};
await save("summary.json",summary);console.log(JSON.stringify(summary));
