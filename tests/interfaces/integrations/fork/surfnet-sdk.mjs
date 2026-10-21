/**
 * Opt-in driver: Surfpool's own Node SDK (@solana/surfpool and its kit plugin) on a fork, as a developer's Node tests use it.
 * The fork runs in its own process (Surfnet.startWithConfig blocks Node's event loop until Surfpool is ready) and hands
 * the SDK a user-authored remoteRpcUrl and matchDatasource setting. A token account the developer did not declare is refused
 * and named; restarting the fork with --new-account, as the error says, makes the same call work.
 * SDK_DIR: a directory where @solana/surfpool and @solana/kit resolve (the published package, or a Surfpool checkout's
 * crates/sdk-node). Usage:
 *   API_ALCHEMY=… node surfnet-sdk.mjs SDK_ENTRY INSTALLATION SURFPOOL SDK_DIR DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE
 */
import assert from "node:assert/strict";
import {spawnSync} from "node:child_process";
import {createRequire} from "node:module";
import {mkdtemp} from "node:fs/promises";
import {tmpdir} from "node:os";
import {resolve,dirname,join} from "node:path";
import {open,output,closed,grade,parse,SUPPORT_ACCOUNTS} from "./shared.mjs";
import {clockTime} from "./flows-lib.mjs";
const [entry,installation,surfpoolBinary,sdkDir,dataDir,alchemyConfig,outArg,signature]=process.argv.slice(2);
if(signature===undefined)throw new Error("usage: API_ALCHEMY=… node surfnet-sdk.mjs SDK_ENTRY INSTALLATION SURFPOOL SDK_DIR DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const require=createRequire(join(resolve(sdkDir),"package.json"));
const {Surfnet}=require("@solana/surfpool"),{createClient,address}=require("@solana/kit"),{surfpool:kitPlugin}=require("@solana/surfpool/kit");
// One fork per port: a start stops whatever fork already serves its port, so each mode gets its own.
const launcher=resolve(dirname(resolve(entry)),"../launcher.mjs"),save=await output(outArg),port="0";let forkUrl,forkRunning=false;
const CLOCK="SysvarC1ock11111111111111111111111111111111",checks={};
const home=await mkdtemp(join(tmpdir(),"surfnet-sdk-home-"));
const json=text=>text.trim().split("\n").filter(line=>line.startsWith("{")).map(line=>JSON.parse(line));
const forkCli=(...args)=>{
    const r=spawnSync(process.execPath,[launcher,"fork",...args,"--port",port,"--data-dir",resolve(dataDir),
        "--alchemy-config",resolve(alchemyConfig)],{encoding:"utf8",timeout:900000,cwd:home,env:{...process.env,HOME:home,XDG_RUNTIME_DIR:home}});
    assert.equal(r.status,0,`svm-replay fork ${args[0]} failed: ${r.stderr.slice(-600)}`);return json(r.stdout);
};
async function rpc(url,method,params=[]){
    const response=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})});
    const body=await response.json();if(body.error)throw new Error(`${method}: ${body.error.message}`);return body.result;
}
const balance=async(surfnet,key)=>String((await rpc(surfnet.rpcUrl,"getBalance",[key,{commitment:"processed"}])).value);

// What the developer's test asserts against: SVM Replay's verified result of the transaction, read through the SDK.
const {replay,loadClosedBoundary,base58Encode,programDataAddress}=await open(entry,installation,dataDir,alchemyConfig);
const boundary=await loadClosedBoundary(replay,{tx:signature},[]),original=boundary.expected.original,tx=boundary.transactionBase64;
const payer=parse(tx,base58Encode).keys[0],before=key=>original.accountTransitions.find(t=>t.pubkey===key).before;
const token=original.accountTransitions.find(t=>t.before.tokenAmount!==null),tokenImage=boundary.accounts.get(token.pubkey);
const mint=base58Encode(Buffer.from(tokenImage.dataBase64,"base64").subarray(0,32)),tokenProgram=tokenImage.owner;
const recipient=Surfnet.newKeypair().publicKey;
process.chdir(home);

/** The user configures the embedded Surfnet directly; the historical server only supplies RPC state. */
async function startBoth(newAccounts){
    const legacyData=SUPPORT_ACCOUNTS.filter(key=>{const a=boundary.accounts.get(key);return a!==undefined&&a.executable&&a.owner!=="BPFLoaderUpgradeab1e11111111111111111111111";}).map(programDataAddress);
    const [started]=forkCli("start",signature,"--detach","--exit-with",String(process.pid),
        ...SUPPORT_ACCOUNTS.flatMap(key=>["--also",key]),...[...newAccounts,...legacyData].flatMap(key=>["--new-account",key]));
    forkUrl=started.url;forkRunning=true;
    return Surfnet.startWithConfig({remoteRpcUrl:forkUrl,matchDatasource,blockProductionMode:"transaction"});
}
async function stopBoth(surfnet){
    surfnet.stop();const [status]=forkCli("stop");forkRunning=false;
    for(let i=0;i<40&&!await closed(new URL(forkUrl).port);i++)await new Promise(r=>setTimeout(r,250));
    assert.ok(await closed(new URL(forkUrl).port),"the fork is still listening");return status;
}

let surfnet;
try {
surfnet=await startBoth([recipient]);
// 1. Historical reads, and the original transaction reaches its verified result. Surfpool's Node SDK has no option to
// skip blockhash checks, so the simulation replaces the historical blockhash; signatures are not verified.
assert.equal(await balance(surfnet,payer),before(payer).lamports);
const {value:simulation}=await rpc(surfnet.rpcUrl,"simulateTransaction",[tx,{encoding:"base64",sigVerify:false,replaceRecentBlockhash:true}]);
if(matchDatasource){assert.equal(grade(simulation,original),"G2");checks.original="G2";}
else checks.stockLimitation="Stock Surfpool keeps today's Clock and bundled programs; execution parity not asserted.";

// 2. The SDK's cheatcodes on historical and declared new accounts.
surfnet.fundSol(recipient,2_000_000_000);assert.equal(await balance(surfnet,recipient),"2000000000");
surfnet.setAccount(payer,42,new Uint8Array(),"11111111111111111111111111111111");assert.equal(await balance(surfnet,payer),"42");
surfnet.resetAccount(payer);assert.equal(await balance(surfnet,payer),before(payer).lamports);checks.cheatcodes=true;

// 3. The kit plugin in attach mode on the same Surfnet: reads and cheatcodes through @solana/kit.
const client=await createClient().use(kitPlugin({rpcUrl:surfnet.rpcUrl}));
assert.equal(String((await client.rpc.getBalance(address(payer),{commitment:"processed"}).send()).value),before(payer).lamports);
await client.cheatcodes.setAccount(payer,{lamports:7}).send();assert.equal(await balance(surfnet,payer),"7");
await client.cheatcodes.resetAccount(payer).send();assert.equal(await balance(surfnet,payer),before(payer).lamports);checks.kit=true;

// 4. Time travel from the historical moment needs the datasource's Clock (stock Surfpool keeps wall-clock time).
if(matchDatasource){
    const historical=clockTime(boundary.clock.dataBase64);
    surfnet.timeTravelToTimestamp(Number(historical+86_400n)*1000);
    const {value}=await rpc(surfnet.rpcUrl,"getAccountInfo",[CLOCK,{encoding:"base64",commitment:"processed"}]);
    const time=clockTime(value.data[0]);assert.ok(time>=historical+86_400n&&time<=historical+86_460n,`clock ${time}`);checks.timeTravel=true;
}

// 5. A token account the developer did not declare is refused, and the fork names it.
const ata=surfnet.getAta(recipient,mint,tokenProgram);
assert.throws(()=>surfnet.fundToken(recipient,mint,1000,tokenProgram));
assert.ok((await rpc(forkUrl,"svmReplayFork_status")).misses.includes(ata),"the refused token account is not named");
await stopBoth(surfnet);surfnet=undefined;

// 6. Restarted with --new-account for it, as the error says: the same call works.
surfnet=await startBoth([recipient,ata]);
surfnet.fundToken(recipient,mint,1000,tokenProgram);
assert.equal((await rpc(surfnet.rpcUrl,"getTokenAccountBalance",[ata,{commitment:"processed"}])).value.amount,"1000");checks.declared=true;
const forkStatus=await stopBoth(surfnet);surfnet=undefined;surfnet=undefined;
await save("summary.json",{status:"PASS",matchDatasource,signature,sdk:require("@solana/surfpool/package.json").version,checks,forkStatus});
console.log(JSON.stringify({status:"PASS",matchDatasource,checks}));

} finally {
    if(surfnet!==undefined)surfnet.stop();
    if(forkRunning)forkCli("stop");
}
