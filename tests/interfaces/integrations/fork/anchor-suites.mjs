/**
 * Opt-in driver: Anchor's own test projects, unchanged, under `anchor test` on a fork just before a real mainnet
 * transaction, each compared with the same build on Anchor's default local Surfpool. Only [surfpool] and [hooks] change,
 * to the user-authored datasource URL and historical server hooks; Anchor starts Surfpool.
 * Development mode: the tests create accounts at fresh keypairs, and programs built today may use sBPF formats newer
 * than the fork's history; both are reported. A test that passes locally but fails on the fork is a branch to explain.
 * Usage: API_ALCHEMY=… node anchor-suites.mjs SDK_ENTRY SURFPOOL ANCHOR AGAVE_BIN ANCHOR_REPO JS_DIR DATA_DIR ALCHEMY_CONFIG
 *        NEW_OUT SIGNATURE PROJECT...   (ANCHOR_REPO: a disposable checkout; JS_DIR: where @anchor-lang/core, mocha, … are installed)
 */
import {spawnSync} from "node:child_process";
import {mkdir,writeFile,readFile,symlink,lstat} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {anchorConfig,anchorPorts,prepareProgramKeys,buildAnchor,output,closed} from "./shared.mjs";
const [entry,surfpoolBinary,anchorBinary,agaveBin,anchorRepo,jsDir,dataDir,alchemyConfig,outArg,signature,...projects]=process.argv.slice(2);
if(projects.length===0)throw new Error("usage: see the header of anchor-suites.mjs");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const arch=process.env.SVM_REPLAY_FORK_PROGRAM_ARCH===undefined?"v3":process.env.SVM_REPLAY_FORK_PROGRAM_ARCH;
const {port:forkPort,rpcPort,wsPort}=await anchorPorts();
const launcher=resolve(dirname(resolve(entry)),"../launcher.mjs"),out=resolve(outArg),save=await output(out),shims=join(out,"bin");
await mkdir(shims);
// The commands Anchor's hooks and scripts call by name, as an npm installation provides them.
await writeFile(join(shims,"svm-replay"),`#!/bin/sh\nexec node ${launcher} "$@"\n`,{mode:0o755});
await symlink(resolve(surfpoolBinary),join(shims,"surfpool"));await symlink(resolve(anchorBinary),join(shims,"anchor"));
const toolPath=`${shims}:${resolve(agaveBin)}:${join(resolve(jsDir),"node_modules/.bin")}:${process.env.PATH}`;
const exists=async path=>{try {await lstat(path);return true;} catch {return false;}};
// `anchor keys sync` rewrites Anchor.toml in normalized form: replace whole sections by name.
const sections=text=>new Map(text.split(/^(?=\[[a-z_.]+\]\s*$)/m).map(part=>[part.match(/^\[([a-z_.]+)\]/)?.[1]??"",part]));
/** mocha's summary: passing, failing and pending counts, and the titles of the failures. */
function summary(log){
    const count=word=>{const match=log.match(new RegExp(`(\\d+) ${word}`));return match===null?undefined:Number(match[1]);};
    if(count("passing")===undefined)return {passing:null,failing:null,pending:null,failures:[],summaryError:"Anchor did not produce a mocha test summary"};
    const failures=[...log.matchAll(/^ {2}\d+\) (.+)$/gm)].map(m=>m[1].trim());
    return {passing:count("passing"),failing:count("failing")===undefined?0:count("failing"),pending:count("pending")===undefined?0:count("pending"),failures:[...new Set(failures)]};
}
async function released(){for(let i=0;i<60;i++){if(await closed(forkPort)&&await closed(rpcPort))return true;await new Promise(r=>setTimeout(r,500));}return false;}

// One throwaway wallet (Surfpool's Anchor deployment runbook signs with ~/.config/solana/id.json), one printed fork config.
const home=join(out,"home");await mkdir(join(home,".config/solana"),{recursive:true});
const tool=(file,args)=>spawnSync(file,args,{env:{...process.env,PATH:toolPath},encoding:"utf8",timeout:1800000});
tool("solana-keygen",["new","--no-bip39-passphrase","-s","-o",join(home,".config/solana/id.json"),"--force"]);
const wallet=tool("solana-keygen",["pubkey",join(home,".config/solana/id.json")]).stdout.trim();
const generated=sections(anchorConfig({signature,dataDir,alchemyConfig,wallet,matchDatasource,port:forkPort,rpcPort,wsPort}));
const results=[];
for(const project of projects){
    const dir=join(resolve(anchorRepo),"tests",project),logs=join(out,project);await mkdir(logs);
    // Builds use the developer's HOME (cached platform tools); test runs a throwaway one whose default keypair is the wallet.
    const run=(file,args,timeout=1800000,env={...process.env,PATH:toolPath,CARGO_TARGET_DIR:join(dir,"target")})=>spawnSync(file,args,{cwd:dir,env,encoding:"utf8",timeout});
    for(const link of [join(dir,"node_modules"),join(resolve(anchorRepo),"tests/node_modules")])
        if(!await exists(link))await symlink(join(resolve(jsDir),"node_modules"),link);
    await prepareProgramKeys(dir,{...process.env,PATH:toolPath,CARGO_TARGET_DIR:join(dir,"target")});
    const sync=run("anchor",["keys","sync"]),build=await buildAnchor(dir,{...process.env,PATH:toolPath,CARGO_TARGET_DIR:join(dir,"target")},arch);  // as Anchor's own CI builds them
    await writeFile(join(logs,"build.log"),`${sync.stdout}\n${sync.stderr}\n${build.stdout}\n${build.stderr}`);
    if(build.status!==0){results.push({project,build:"failed"});console.log(JSON.stringify(results.at(-1)));continue;}
    const synced=await readFile(join(dir,"Anchor.toml"),"utf8");
    const attempt=async name=>{
        const result=run("anchor",["test","--skip-build"],900000,{...process.env,PATH:toolPath,CARGO_TARGET_DIR:join(dir,"target"),HOME:home}),log=`${result.stdout}\n${result.stderr}`;
        await writeFile(join(logs,`${name}.log`),log);
        const status=log.split("\n").find(line=>line.startsWith('{"signature":')&&line.includes('"derivedFeatures":'));
        return {exitCode:result.status,released:await released(),...summary(log),fork:status===undefined?null:JSON.parse(status)};
    };
    // 1. Anchor's default: a local Surfpool (offline), the project's own Anchor.toml.
    const local=await attempt("local");
    // 2. The same build with user-authored [surfpool] and [hooks].
    const toml=sections(synced);
    for(const name of ["surfpool","hooks"])toml.set(name,`${generated.get(name).trim()}\n\n`);
    await writeFile(join(dir,"Anchor.toml"),[...toml.values()].join(""));await writeFile(join(logs,"Anchor.fork.toml"),[...toml.values()].join(""));
    const onFork=await attempt("fork");
    await writeFile(join(dir,"Anchor.toml"),synced);
    const regressions=onFork.failures.filter(title=>!local.failures.includes(title));
    results.push({project,local,fork:onFork,regressions});console.log(JSON.stringify({project,local:[local.passing,local.failing],
        fork:[onFork.passing,onFork.failing],regressions,assumed:onFork.fork?.assumedAbsent.length??null}));
}
const regressed=results.filter(r=>r.build==="failed"||r.local.summaryError!==undefined||r.fork.summaryError!==undefined||!r.local.released||!r.fork.released||r.regressions.length>0||r.fork.passing<r.local.passing);
await save("summary.json",{matchDatasource,signature,projects:results,regressed:regressed.map(r=>r.project)});
console.log(JSON.stringify({status:regressed.length===0?"PASS":"FAIL",matchDatasource,projects:results.length,regressed:regressed.map(r=>r.project)}));
process.exitCode=regressed.length===0?0:1;
