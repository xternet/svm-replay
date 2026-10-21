#!/usr/bin/env node
// svm-replay fork: verified historical state served as a local Solana RPC URL.
import {spawn} from "node:child_process";
import {readFile,writeFile,rm,open,mkdir,lstat} from "node:fs/promises";
import {homedir} from "node:os";
import {join} from "node:path";
import {parseArgs} from "node:util";
import {Replay} from "../../index.js";
import {startHistorical} from "./index.js";
import {parseExact,stringifyExact} from "./json.js";
import {forkHelp} from "./help.js";

const options={port:{type:"string",default:"8999"},"allow-new-accounts":{type:"boolean"},
    also:{type:"string",multiple:true,default:[]},"new-account":{type:"string",multiple:true,default:[]},detach:{type:"boolean"},
    "data-dir":{type:"string"},"alchemy-config":{type:"string"},help:{type:"boolean",short:"h"},"exit-with":{type:"string"}};
const help=()=>forkHelp(process.stdout.isTTY===true&&process.env.NO_COLOR===undefined);
let activationPath;
/**
 * The pid, ready and log files live in a folder only this user can write: in a shared temporary folder, another user
 * could plant a pid file and so choose which process `stop` kills.
 */
async function stateDirectory(){
    // $XDG_RUNTIME_DIR only when it exists and is this user's (SSH sessions and containers often set it without one).
    const runtime=process.env.XDG_RUNTIME_DIR;
    const ours=runtime!==undefined&&runtime!==""&&await lstat(runtime).then(s=>s.isDirectory()&&(process.getuid===undefined||s.uid===process.getuid()),()=>false);
    const base=ours?runtime:process.platform==="win32"?process.env.LOCALAPPDATA:join(homedir(),process.platform==="darwin"?"Library/Application Support":".local/state");
    if(base===undefined)throw new Error("no private folder for the fork's state (set XDG_RUNTIME_DIR or LOCALAPPDATA)");
    const directory=join(base,"svm-replay","fork");
    await mkdir(directory,{recursive:true,mode:0o700});
    const stats=await lstat(directory);
    if(!stats.isDirectory()||(process.getuid!==undefined&&stats.uid!==process.getuid())||(process.platform!=="win32"&&(stats.mode&0o077)!==0))
        throw new Error(`${directory} must be a folder of this user that only this user can access`);
    return directory;
}
let STATE;
const file=(port,kind)=>join(STATE,`${port}.${kind}`);
const report=value=>console.log(JSON.stringify(value));
const sleep=ms=>new Promise(r=>setTimeout(r,ms));

/** Stops a fork left on this port (Anchor skips post_test when a test fails) and waits until it has exited. */
async function stopRunning(port){
    const stored=await optionalFile(file(port,"pid"));if(stored===null)return null;
    const pid=Number(stored);if(!Number.isSafeInteger(pid)||pid<=0)throw new Error(`invalid fork pid in ${file(port,"pid")}`);
    const alive=async()=>{try {
        process.kill(pid,0);
        if(process.platform==="linux"){
            const stat=await readFile(`/proc/${pid}/stat`,"utf8");
            return stat.slice(stat.lastIndexOf(")")+2,stat.lastIndexOf(")")+3)!=="Z";
        }
        return true;
    } catch(error){if(error.code==="ESRCH"||error.code==="ENOENT")return false;throw error;}};
    if(await alive()){process.kill(pid,"SIGTERM");for(let i=0;i<60&&await alive();i++)await sleep(250);if(await alive())process.kill(pid,"SIGKILL");}
    while(await alive())await sleep(100);
    await rm(file(port,"pid"),{force:true});await rm(file(port,"ready"),{force:true});return pid;
}
async function optionalFile(path){
    try {return await readFile(path,"utf8");} catch(error){if(error.code==="ENOENT")return null;throw error;}
}
/** Detach only once the historical RPC server is ready, or report startup failure. */
async function detach(values){
    await stopRunning(values.port);
    const logPath=file(values.port,"log"),log=await open(logPath,"w");
    const child=spawn(process.execPath,[process.argv[1],...process.argv.slice(2).filter(a=>a!=="--detach")],{detached:true,stdio:["ignore",log.fd,log.fd]});
    await log.close();
    await writeFile(file(values.port,"pid"),String(child.pid));
    let ready;
    for(const deadline=Date.now()+15*60000;;await sleep(500)){
        if(child.exitCode!==null){
            const reason=(await readFile(logPath,"utf8")).split("\n").filter(line=>line.startsWith("SVM_REPLAY_FORK: ")).at(-1);
            throw new Error(`fork exited with ${child.exitCode}: ${reason===undefined?"no error reason in log":reason.slice(17)} (log: ${logPath})`);
        }
        const text=await optionalFile(file(values.port,"ready"));
        if(text!==null){ready=JSON.parse(text);break;}
        if(Date.now()>deadline){await stopRunning(values.port);throw new Error(`fork not ready after 15 minutes; see ${logPath}`);}
    }
    child.unref();report({detached:child.pid,url:ready.url,log:logPath});
}
async function openReplay(values){
    const {directory}=JSON.parse(await readFile(activationPath(),"utf8"));
    const pin=JSON.parse(await readFile(join(directory,"installation.json"),"utf8"));
    return Replay.open({bundlePath:join(directory,"bundle.json"),bundleSha256:pin.bundleSha256,dataDir:values["data-dir"],alchemyConfig:values["alchemy-config"]});
}
async function rpc(url,method,params,timeoutMs=120000){
    const response=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},signal:AbortSignal.timeout(timeoutMs),
        body:stringifyExact({jsonrpc:"2.0",id:1,method,params})});
    const body=parseExact(await response.text());
    if(body.error!==undefined)throw new Error(`${method}: ${body.error.message}`);
    return body.result;
}
async function fork(signature,values){
    return startHistorical(await openReplay(values),{tx:signature,port:Number(values.port),newAccounts:values["new-account"],
        also:values.also,allowNewAccounts:values["allow-new-accounts"]===true});
}
const summary=built=>({url:built.datasource.url,signature:built.boundary.signature,slot:built.boundary.targetSlot,
    runtime:built.boundary.runtime===undefined||built.boundary.runtime===null?null:built.boundary.runtime.name,family:built.boundary.family,predecessors:built.predecessors,
    accounts:built.datasource.state.size,unavailable:[...built.boundary.unavailable],activeFeatures:built.boundary.features.length});
export async function runForkCli(installationPath){
    activationPath=installationPath;
    const argv=process.argv[2]==="help"&&process.argv[3]==="fork"?["--help",...process.argv.slice(4)]:
        process.argv.slice(process.argv[2]==="fork"?3:2);
    const {values,positionals}=parseArgs({options,allowPositionals:true,strict:true,args:argv});
    const [command,signature]=positionals;
    if(values.help||command===undefined){console.log(help());return;}
    if(command!=="start"&&command!=="stop")throw new Error(`unknown command ${command}\n${help()}`);
    if(positionals.length>(command==="start"?2:1))throw new Error(`unexpected arguments for ${command}`);
    if(!/^\d+$/.test(values.port)||Number(values.port)>65535)throw new Error("--port must be an integer from 0 to 65535");
    if(values["exit-with"]!==undefined&&(!/^\d+$/.test(values["exit-with"])||Number(values["exit-with"])<=0||!Number.isSafeInteger(Number(values["exit-with"]))))
        throw new Error("--exit-with needs a positive process id");
    if(command==="start"&&signature===undefined)throw new Error("start needs a transaction signature");
    STATE=await stateDirectory();
    if(command==="stop"){
        const text=await optionalFile(file(values.port,"ready"));
        const url=text===null?`http://127.0.0.1:${values.port}`:JSON.parse(text).url;
        try {report(await rpc(url,"svmReplayFork_status",[]));}
        catch(error){console.error(`SVM_REPLAY_FORK: status at ${url}: ${error.message}`);}
        report({stopped:await stopRunning(values.port)});return;
    }
    if(values.detach)return detach(values);
    const built=await fork(signature,values);
    const shutdown=async()=>{await built.close();await rm(file(values.port,"pid"),{force:true});await rm(file(values.port,"ready"),{force:true});process.exit(0);};
    report(summary(built));
    // Anchor can skip post_test when a test fails; the server exits with the process that ran its hook.
    if(values["exit-with"]!==undefined){
        const owner=Number(values["exit-with"]);
        setInterval(()=>{try {process.kill(owner,0);} catch(error){
            if(error.code==="ESRCH")process.kill(process.pid,"SIGTERM");
            else {console.error(`SVM_REPLAY_FORK: parent ${owner}: ${error.message}`);process.kill(process.pid,"SIGTERM");}
        }},1000).unref();
    }
    process.on("SIGINT",shutdown);process.on("SIGTERM",shutdown);
    await writeFile(file(values.port,"pid"),String(process.pid));
    await writeFile(file(values.port,"ready"),JSON.stringify({pid:process.pid,url:built.datasource.url}));
}
