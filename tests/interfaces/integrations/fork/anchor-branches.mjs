/**
 * Opt-in driver: two Anchor branches on a fork, with the examples/anchor/historical workspace.
 * 1. `anchor localnet` (no hooks run): the developer starts the fork, then Anchor starts Surfpool on it and deploys the
 *    workspace; the node serves the verified state and the deployed program sees the historical clock and account.
 * 2. A program built today (sBPF v3) deployed to a fork from before mainnet activated v3, with historical feature matching:
 *    the outcome is recorded, and must not be a silent pass.
 * Usage: API_ALCHEMY=… node anchor-branches.mjs SDK_ENTRY INSTALLATION SURFPOOL ANCHOR AGAVE_BIN DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE
 */
import assert from "node:assert/strict";
import {spawn,spawnSync} from "node:child_process";
import {cp,mkdir,writeFile,readFile,symlink} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {fileURLToPath} from "node:url";
import {anchorConfig,anchorPorts,prepareProgramKeys,buildAnchor,open,output,closed,hash} from "./shared.mjs";
const [entry,installation,surfpoolBinary,anchorBinary,agaveBin,dataDir,alchemyConfig,outArg,signature]=process.argv.slice(2);
if(signature===undefined)throw new Error("usage: see the header of anchor-branches.mjs");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const arch=process.env.SVM_REPLAY_FORK_PROGRAM_ARCH===undefined?"v3":process.env.SVM_REPLAY_FORK_PROGRAM_ARCH;
const {port:forkPort,rpcPort,wsPort}=await anchorPorts();
const here=dirname(fileURLToPath(import.meta.url)),example=resolve(here,"../../../../examples/anchor/historical");
const launcher=resolve(dirname(resolve(entry)),"../launcher.mjs"),out=resolve(outArg),save=await output(out);
const work=join(out,"work"),shims=join(out,"bin"),home=join(out,"home");
await cp(example,work,{recursive:true});await mkdir(shims);await mkdir(join(home,".config/solana"),{recursive:true});
await writeFile(join(shims,"svm-replay"),`#!/bin/sh\nexec node ${launcher} "$@"\n`,{mode:0o755});
await symlink(resolve(surfpoolBinary),join(shims,"surfpool"));await symlink(resolve(anchorBinary),join(shims,"anchor"));
const env={...process.env,CARGO_TARGET_DIR:join(work,"target"),PATH:`${shims}:${resolve(agaveBin)}:${process.env.PATH}`},testEnv={...env,HOME:home};
const run=(file,args,options={})=>spawnSync(file,args,{cwd:work,env,encoding:"utf8",timeout:1800000,...options});
run("solana-keygen",["new","--no-bip39-passphrase","-s","-o","wallet.json","--force"]);
await cp(join(work,"wallet.json"),join(home,".config/solana/id.json"));
const wallet=run("solana-keygen",["pubkey","wallet.json"]).stdout.trim();
await prepareProgramKeys(work,env);
run("anchor",["keys","sync"]);const builtProgram=await buildAnchor(work,env,arch);assert.equal(builtProgram.status,0,`anchor build failed: ${builtProgram.stderr}`);
const programId=run("solana-keygen",["pubkey","target/deploy/historical_probe-keypair.json"]).stdout.trim();
const {fork:adapter,replay}=await open(entry,installation,dataDir,alchemyConfig);
const boundary=await adapter.loadBoundary(replay,{tx:signature}),original=boundary.expected.original;
const known=original.accountTransitions.find(t=>t.before.presence==="present"&&!t.before.executable);
async function rpc(method,params=[]){
    const response=await fetch(`http://127.0.0.1:${rpcPort}`,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})});
    const body=await response.json();if(body.error)throw new Error(`${method}: ${body.error.message}`);return body.result;
}
const sections=text=>new Map(text.split(/^(?=\[[a-z_.]+\]\s*$)/m).map(part=>[part.match(/^\[([a-z_.]+)\]/)?.[1]??"",part]));
/** User-authored Anchor configuration: Anchor owns Surfpool; fork hooks own the historical RPC server. */
async function configure(test){
    const config=anchorConfig({signature,dataDir,alchemyConfig,wallet,matchDatasource,port:forkPort,rpcPort,wsPort});
    const toml=sections(await readFile(join(work,"Anchor.toml"),"utf8")),generated=sections(config);
    for(const name of ["surfpool","hooks"])toml.set(name,`${generated.get(name).trim()}\n\n`);
    if(test!==undefined)toml.set("scripts",`[scripts]\ntest = ${JSON.stringify(test)}\n\n`);
    await writeFile(join(work,"Anchor.toml"),[...toml.values()].join(""));
    return JSON.parse(/^pre_test\s*=\s*(".*")\s*$/m.exec(generated.get("hooks"))[1]);
}
const released=async()=>{for(let i=0;i<60;i++){if(await closed(forkPort)&&await closed(rpcPort))return true;await new Promise(r=>setTimeout(r,500));}return false;};
const checks={};

// 1. anchor localnet: the fork first (the printed pre_test command, with this shell as its owner), then Anchor.
const start=(await configure()).replace("$PPID",String(process.pid));
assert.equal(spawnSync("bash",["-c",start],{cwd:work,env:testEnv,encoding:"utf8",timeout:900000}).status,0,"the fork did not start");
// Anchor's localnet runs until a line arrives on its stdin (Enter), then stops its Surfpool.
const localnet=spawn("anchor",["localnet","--skip-build"],{cwd:work,env:testEnv,stdio:["pipe","pipe","pipe"]});
const exited=new Promise(r=>localnet.once("exit",(code,signal)=>r({code,signal})));
let log="";localnet.stdout.on("data",d=>{log+=d;});localnet.stderr.on("data",d=>{log+=d;});
try {
    let deployed=null;
    for(let i=0;i<360&&deployed===null;i++){
        await new Promise(r=>setTimeout(r,500));
        deployed=await rpc("getAccountInfo",[programId,{encoding:"base64"}]).then(r=>r.value?.executable?r.value:null,()=>null);
    }
    assert.ok(deployed!==null,`anchor localnet did not deploy the workspace: ${log.slice(-800)}`);
    const {value}=await rpc("getAccountInfo",[known.pubkey,{encoding:"base64"}]);
    assert.equal(String(value.lamports),known.before.lamports);assert.equal(hash(Buffer.from(value.data[0],"base64")),known.before.dataHash);
    assert.ok(await rpc("getSlot",[{commitment:"processed"}])>=boundary.targetSlot,"the node is not at the historical slot");checks.localnet=true;
} finally {
    localnet.stdin.end("\n");checks.localnetExit=await exited;
    run("svm-replay",["fork","stop","--port",String(forkPort)],{env:testEnv});
    await writeFile(join(out,"localnet.log"),log);
}
const holders=()=>spawnSync("ss",["-ltnp"],{encoding:"utf8"}).stdout.split("\n").filter(l=>/:(8999|8899) /.test(l)).join(" | ");
assert.ok(await released(),`localnet left a fork or Surfpool running: ${holders()}`);

// 2. Deploy a program built today to a fork from before mainnet activated sBPF v3, with historical feature matching. The
// test script only reports whether the program is deployed and what invoking it returns.
const modern=await buildAnchor(work,env,"v3");assert.equal(modern.status,0,modern.stderr);
const PROBE=`import {createHash} from "node:crypto";
const url=process.env.ANCHOR_PROVIDER_URL,[program,account,payer]=process.argv.slice(2),A="${"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"}";
const decode=t=>{let n=0n;for(const c of t)n=n*58n+BigInt(A.indexOf(c));return Buffer.from(n.toString(16).padStart(64,"0"),"hex");};
const rpc=async(method,params)=>(await fetch(url,{method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})})).json();
const {result:{value:{blockhash}}}=await rpc("getLatestBlockhash",[]),data=createHash("sha256").update("global:observe").digest().subarray(0,8);
const message=Buffer.concat([Buffer.from([1,0,2,3]),...[payer,account,program].map(decode),decode(blockhash),Buffer.from([1,2,1,1,data.length]),data]);
const simulation=await rpc("simulateTransaction",[Buffer.concat([Buffer.from([1]),Buffer.alloc(64),message]).toString("base64"),
    {encoding:"base64",sigVerify:false,replaceRecentBlockhash:true}]);
const info=await rpc("getAccountInfo",[program,{encoding:"base64"}]);
console.log("PROBE "+JSON.stringify({deployed:info.result?.value?.executable??false,err:simulation.result?.value?.err??simulation.error??null,
    logs:simulation.result?.value?.logs?.slice(0,4)??[]}));
`;
await writeFile(join(work,"tests/probe.mjs"),PROBE);
await configure(`node tests/probe.mjs ${programId} ${known.pubkey} ${wallet}`);
const test=spawnSync("anchor",["test","--skip-build"],{cwd:work,env:testEnv,encoding:"utf8",timeout:900000});
const deployLog=`${test.stdout}\n${test.stderr}`;await writeFile(join(out,"deploy-without-new-formats.log"),deployLog);
const probe=/^PROBE (.*)$/m.exec(deployLog),outcome=probe===null?null:JSON.parse(probe[1]);
checks.historicalFeatureDeployment={exitCode:test.status,released:await released(),probe:outcome,
    deployError:outcome===null?deployLog.split("\n").filter(line=>/rror|sbpf|SBPF|ELF|nvalid/.test(line)).slice(0,6):undefined};
// The developer must see it: the deployment fails, or the program is missing or fails when invoked, never a normal run.
if(matchDatasource)assert.ok(outcome===null||!outcome.deployed||outcome.err!==null,`a program the historical runtime cannot load ran: ${JSON.stringify(outcome)}`);
await save("summary.json",{status:"PASS",matchDatasource,signature,checks});
console.log(JSON.stringify({status:"PASS",matchDatasource,checks}));
