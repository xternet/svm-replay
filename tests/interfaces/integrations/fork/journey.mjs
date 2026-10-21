/** Developer flow: fork start/stop serve history; the user starts and owns Surfpool separately. Cached fixtures only. */
import assert from "node:assert/strict";
import {spawn,spawnSync} from "node:child_process";
import {mkdtemp} from "node:fs/promises";
import {tmpdir} from "node:os";
import {resolve,dirname,join} from "node:path";
import {open,output,closed,hash,rpc,startSurfpool,SUPPORT_ACCOUNTS,exact,grade} from "./shared.mjs";
const [entry,installation,surfpoolBinary,dataDir,alchemyConfig,outArg,failedSignature,unsupportedSignature]=process.argv.slice(2);
if(!unsupportedSignature)throw new Error("usage: journey.mjs SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT FAILED_SIGNATURE UNSUPPORTED_SIGNATURE");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const launcher=resolve(dirname(resolve(entry)),"../launcher.mjs"),save=await output(outArg),port="0";
const home=await mkdtemp(join(tmpdir(),"journey-home-"));
const common=["--port",port,"--data-dir",resolve(dataDir),"--alchemy-config",resolve(alchemyConfig)];
const fork=(...args)=>{
    const r=spawnSync(process.execPath,[launcher,"fork",...args,...common],{encoding:"utf8",timeout:args[0]==="stop"?30000:900000,cwd:home,env:{...process.env,HOME:home,XDG_RUNTIME_DIR:home}});
    const json=r.stdout.split("\n").filter(line=>line.startsWith("{")).map(line=>JSON.parse(line));return {...r,json};
};
const {fork:adapter,replay,loadClosedBoundary,programDataAddress}=await open(entry,installation,dataDir,alchemyConfig);
const boundary=await loadClosedBoundary(replay,{tx:failedSignature}),steps={};
const legacyData=SUPPORT_ACCOUNTS.filter(key=>{const a=boundary.accounts.get(key);return a!==undefined&&a.executable&&a.owner!=="BPFLoaderUpgradeab1e11111111111111111111111";}).map(programDataAddress);
const start=()=>fork("start",failedSignature,"--detach",...SUPPORT_ACCOUNTS.flatMap(key=>["--also",key]),...legacyData.flatMap(key=>["--new-account",key]));
const started=start();assert.equal(started.status,0,started.stderr);steps.fork=started.json.at(-1);
let surfpool;
try {
    // The CLI's own RPC serves every historical sysvar byte for byte, with no execution consumer involved.
    const sysvars=[...boundary.accounts.values()].filter(a=>a.owner==="Sysvar1111111111111111111111111111111111111");
    const {value}=await rpc(steps.fork.url,"getMultipleAccounts",[sysvars.map(a=>a.pubkey),{encoding:"base64"}]);
    sysvars.forEach((a,i)=>assert.equal(hash(Buffer.from(value[i].data[0],"base64")),hash(Buffer.from(a.dataBase64,"base64")),a.pubkey));
    steps.historicalSysvars=sysvars.length;
    const features=await rpc(steps.fork.url,"getProgramAccounts",["Feature111111111111111111111111111111111111"]);
    assert.deepEqual(features.map(a=>a.pubkey),boundary.features.map(f=>f.id));steps.features=features.length;
    surfpool=await startSurfpool({binary:surfpoolBinary,datasource:steps.fork.url,matchDatasource});
    const known=boundary.expected.original.accountTransitions.find(t=>t.before.presence==="present"&&!t.before.executable);
    const read=await rpc(surfpool.url,"getAccountInfo",[known.pubkey,{encoding:"base64"}]);
    assert.equal(hash(Buffer.from(read.value.data[0],"base64")),known.before.dataHash);
    steps.historicalRead=known.pubkey;
    if(matchDatasource){
        const local=(await rpc(surfpool.url,"simulateTransaction",[boundary.transactionBase64,{encoding:"base64",sigVerify:false,replaceRecentBlockhash:false}])).value;
        const checked=await exact(replay,failedSignature,boundary.transactionBase64);
        assert.notEqual(checked.status,"ok");assert.equal(grade(local,checked),"G2");steps.failure="G2";
    } else steps.stockLimitation="Today's Clock and bundled programs; execution parity is not asserted.";
    const stopped=fork("stop");assert.equal(stopped.status,0,stopped.stderr);steps.stop=stopped.json;
    assert.deepEqual(stopped.json[0].derivedFeatures.ids,boundary.features.map(f=>f.id));
    assert.ok(await closed(new URL(steps.fork.url).port));
    assert.equal(await rpc(surfpool.url,"getHealth"),"ok","fork stop must leave the user's Surfpool alone");
    await surfpool.stop();surfpool=undefined;
    const again=start();assert.equal(again.status,0,again.stderr);steps.restart=again.json.at(-1);
    assert.notEqual(steps.restart.detached,steps.fork.detached);
    const replace=start();assert.equal(replace.status,0,replace.stderr);steps.replace=replace.json.at(-1);
    assert.notEqual(steps.replace.detached,steps.restart.detached);
    assert.ok(await closed(new URL(steps.restart.url).port));
} finally {
    if(surfpool!==undefined)await surfpool.stop();
    const stopped=fork("stop");assert.equal(stopped.status,0,stopped.stderr);
}
// A foreground start is also owned by fork stop, with no consumer process involved.
const foreground=spawn(process.execPath,[launcher,"fork","start",failedSignature,...common,
    ...SUPPORT_ACCOUNTS.flatMap(key=>["--also",key])],{cwd:home,env:{...process.env,HOME:home,XDG_RUNTIME_DIR:home},stdio:["ignore","pipe","pipe"]});
let foregroundOut="",foregroundError="";
foreground.stdout.on("data",b=>{foregroundOut+=b;});foreground.stderr.on("data",b=>{foregroundError+=b;});
const foregroundDone=new Promise(resolve=>foreground.once("exit",resolve));
try {
    for(const deadline=Date.now()+120000;!foregroundOut.includes("\n");){
        if(foreground.exitCode!==null||Date.now()>deadline)throw new Error(`foreground fork did not start: ${foregroundError}`);
        await new Promise(r=>setTimeout(r,100));
    }
    const ready=JSON.parse(foregroundOut.split("\n")[0]);
    await new Promise(r=>setTimeout(r,250));
    const stopped=fork("stop");assert.equal(stopped.status,0,stopped.stderr);
    assert.ok(await closed(new URL(ready.url).port),"fork stop must also stop a foreground server");
    steps.foregroundStopped=true;
} finally {
    if(foreground.exitCode===null)foreground.kill("SIGTERM");
    await foregroundDone;
}
for(const args of [["start",unsupportedSignature],["start",unsupportedSignature,"--detach"]]){
    const refused=fork(...args);assert.notEqual(refused.status,0);assert.match(refused.stderr,/SVM_REPLAY_FORK: /);
    steps[args.join(" ")]=refused.stderr.trim().split("\n").at(-1).slice(0,300);
}
await save("summary.json",{status:"PASS",matchDatasource,steps});console.log(JSON.stringify({status:"PASS",matchDatasource,steps}));
