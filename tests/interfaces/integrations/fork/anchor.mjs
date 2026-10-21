/**
 * Opt-in historical driver: `anchor test` on Surfpool forked just before a real mainnet transaction.
 * Runs the examples/anchor/historical workspace with the [surfpool] and [hooks] sections exactly as
 * written by the user: matched runtime expectations must pass, a wrong one must fail, and
 * neither run may leave a fork behind (a failing test skips Anchor's post_test).
 * Needs the Anchor CLI with [surfpool] extra_args, Surfpool, and the Agave CLI tools (solana-keygen, cargo-build-sbf).
 * Set SVM_REPLAY_FORK_MATCH_DATASOURCE=1 for a Surfpool build with --match-datasource.
 */
import assert from "node:assert/strict";
import {execFileSync,spawnSync} from "node:child_process";
import {cp,mkdir,writeFile,readFile,symlink} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {fileURLToPath} from "node:url";
import {anchorConfig,anchorPorts,prepareProgramKeys,buildAnchor,open,output,hash,closed} from "./shared.mjs";
const [entry,installation,surfpoolBinary,anchorBinary,agaveBin,dataDir,alchemyConfig,outArg,signature]=process.argv.slice(2);
if(!signature)throw new Error("usage: API_ALCHEMY=… node anchor.mjs INSTALLED_SDK_ENTRY INSTALLATION SURFPOOL ANCHOR AGAVE_BIN DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const arch=process.env.SVM_REPLAY_FORK_PROGRAM_ARCH===undefined?"v3":process.env.SVM_REPLAY_FORK_PROGRAM_ARCH;
const {port:forkPort,rpcPort,wsPort}=await anchorPorts();
const here=dirname(fileURLToPath(import.meta.url)),example=resolve(here,"../../../../examples/anchor/historical");
const launcher=resolve(dirname(resolve(entry)),"../launcher.mjs"),out=resolve(outArg),save=await output(out);
const work=join(out,"work"),shims=join(out,"bin");
await cp(example,work,{recursive:true});await mkdir(shims);
// The commands Anchor's hooks and scripts call by name, as an npm installation provides them.
await writeFile(join(shims,"svm-replay"),`#!/bin/sh\nexec node ${launcher} "$@"\n`,{mode:0o755});
await symlink(resolve(surfpoolBinary),join(shims,"surfpool"));await symlink(resolve(anchorBinary),join(shims,"anchor"));
const env={...process.env,CARGO_TARGET_DIR:join(work,"target"),PATH:`${shims}:${resolve(agaveBin)}:${process.env.PATH}`};
const run=(file,args,options={})=>execFileSync(file,args,{cwd:work,env,encoding:"utf8",stdio:["ignore","pipe","pipe"],...options});

// Throwaway wallet and program key, created locally; never funded outside the fork.
run("solana-keygen",["new","--no-bip39-passphrase","-s","-o","wallet.json","--force"]);
const wallet=run("solana-keygen",["pubkey","wallet.json"]).trim();
// Surfpool's Anchor deployment runbook signs with ~/.config/solana/id.json, not Anchor.toml's provider wallet:
// give each anchor test run a throwaway HOME whose default keypair is the test wallet.
const home=join(out,"home");await mkdir(join(home,".config/solana"),{recursive:true});
await cp(join(work,"wallet.json"),join(home,".config/solana/id.json"));
await prepareProgramKeys(work,env);
run("anchor",["keys","sync"]);
const programId=run("solana-keygen",["pubkey","target/deploy/historical_probe-keypair.json"]).trim();

// Expectations come from SVM Replay's verified boundary, never from the fork under test.
const {fork:adapter,replay}=await open(entry,installation,dataDir,alchemyConfig);
const boundary=await adapter.loadBoundary(replay,{tx:signature}),original=boundary.expected.original;
const before=original.accountTransitions.find(t=>t.before.presence==="present"&&!t.before.executable);
const clock=Buffer.from(boundary.clock.dataBase64,"base64");
const expect={wallet,programId,matchDatasource,account:{pubkey:before.pubkey,dataSha256:before.before.dataHash,lamports:before.before.lamports},
    clock:{unixTimestamp:String(clock.readBigInt64LE(32)),slot:String(clock.readBigUInt64LE(0))},
    original:{transactionBase64:boundary.transactionBase64,status:original.status,normalizedError:original.normalizedError,
        computeUnits:original.computeUnits,logs:original.logs}};
await writeFile(join(work,"expect.json"),JSON.stringify(expect));
const wrongHash=Buffer.from(expect.account.dataSha256,"hex");wrongHash[0]^=1;
await writeFile(join(work,"wrong.json"),JSON.stringify(matchDatasource?{...expect,original:{...expect.original,computeUnits:expect.original.computeUnits+1}}:{...expect,account:{...expect.account,dataSha256:wrongHash.toString("hex")}}));

// Anchor starts Surfpool itself; the hooks serve and stop only historical state.
const config=anchorConfig({signature,dataDir,alchemyConfig,wallet,matchDatasource,port:forkPort,rpcPort,wsPort});
// `anchor keys sync` rewrites Anchor.toml in normalized form: replace whole sections by name.
const sections=text=>new Map(text.split(/^(?=\[[a-z_.]+\]\s*$)/m).map(part=>[part.match(/^\[([a-z_.]+)\]/)?.[1]??"",part]));
const toml=sections(await readFile(join(work,"Anchor.toml"),"utf8")),generated=sections(config);
for(const name of ["surfpool","hooks"])toml.set(name,`${generated.get(name).trim()}\n\n`);
toml.set("scripts",'[scripts]\ntest = "node --test tests/historical.mjs"\n\n');
await writeFile(join(work,"Anchor.toml"),[...toml.values()].join(""));
const builtProgram=await buildAnchor(work,env,arch);
assert.equal(builtProgram.status,0,`anchor build failed: ${builtProgram.stderr}`);

const attempt=async(name,expectFile)=>{
    const result=spawnSync("anchor",["test","--skip-build"],{cwd:work,env:{...env,HOME:home,HISTORICAL_EXPECT:join(work,expectFile)},encoding:"utf8",timeout:900000});
    const log=`${result.stdout}\n${result.stderr}`;
    // No manual cleanup: post_test stops a passing run; a failing run's fork exits with Anchor (--exit-with $PPID).
    let stopped=false;for(let i=0;i<40&&!stopped;i++){stopped=await closed(forkPort)&&await closed(rpcPort);if(!stopped)await new Promise(r=>setTimeout(r,500));}
    const status=log.split("\n").find(line=>line.startsWith('{"signature":')&&line.includes('"derivedFeatures":'));
    return {name,exitCode:result.status,stopped,status:status===undefined?null:JSON.parse(status),logSha256:hash(log),log};
};
const pass=await attempt("verified","expect.json"),fail=await attempt("wrong-expectation","wrong.json");
await writeFile(join(out,"anchor-test-verified.log"),pass.log);await writeFile(join(out,"anchor-test-wrong.log"),fail.log);
await writeFile(join(out,"Anchor.toml"),await readFile(join(work,"Anchor.toml")));
assert.equal(pass.exitCode,0,"anchor test with verified expectations must pass");
assert.notEqual(fail.exitCode,0,"anchor test with a wrong expectation must fail");
assert.ok(pass.stopped&&fail.stopped,"no fork or Surfpool may be left running");
assert.ok(pass.status!==null,"post_test must report what the fork assumed");
// The wrong expectation (compute units + 1) fails exactly the re-run test; every other test passes in both runs.
assert.match(fail.log,matchDatasource?/not ok \d+ - the historical transaction re-runs with the verified outcome/:/not ok \d+ - historical account is the verified image/);
assert.equal((fail.log.match(/^not ok /gm)??[]).length,1);assert.equal((pass.log.match(/^ok /gm)??[]).length,4);
const summary={status:"PASS",signature,family:boundary.family,matchDatasource,wallet,programId,config,
    runs:[pass,fail].map(({log,...rest})=>rest)};
await save("summary.json",summary);console.log(JSON.stringify({...summary,config:undefined}));
