/**
 * Opt-in user-flow matrix: what a developer does with Surfpool on a fork just before a real mainnet transaction.
 * Each flow ends in one branch: "ok" (the historical answer), "explicit" (an error reached the developer) or "silent"
 * (an answer that is not the historical one). A flow passes when its branch is one listed for the Surfpool build.
 * Expected values come from SVM Replay's verified result of the transaction, or from served history checked elsewhere.
 * A "silent" expectation pins a documented Surfpool behaviour (docs/limitations.md), so a change in it is noticed.
 * Usage: API_ALCHEMY=… node flows.mjs SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE PROGRAM_SO
 */
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {startSurfpool,SUPPORT_ACCOUNTS,declareProgramData,open,output,simulate,grade,parse} from "./shared.mjs";
import {keypair,transfer,mintSupply,rentExemption,upgradeAuthority,clockTime,clockSlot,attempt} from "./flows-lib.mjs";
const [entry,installation,surfpoolBinary,dataDir,alchemyConfig,outArg,signature,programFile]=process.argv.slice(2);
if(programFile===undefined)throw new Error("usage: API_ALCHEMY=… node flows.mjs SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE PROGRAM_SO");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1",mode=matchDatasource?"match":"stock";
const {fork,replay,base58Encode,base58Decode,findProgramAddress,programDataAddress}=await open(entry,installation,dataDir,alchemyConfig);
const save=await output(outArg);
const TOKEN_PROGRAMS=["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"];
const ATA="ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",UPGRADEABLE="BPFLoaderUpgradeab1e11111111111111111111111";
const CLOCK="SysvarC1ock11111111111111111111111111111111",RENT="SysvarRent111111111111111111111111111111111";
const MAINNET_GENESIS="5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",SOL=1_000_000_000n;

// Keys a developer creates. The wallet's token account and the clone's ProgramData are declared once their seeds are known.
const [wallet,recipient,clone,stranger,ownProgram,owned]=[0,1,2,3,4,5].map(()=>keypair(base58Encode));
const built=await fork.startHistorical(replay,{tx:signature,also:SUPPORT_ACCOUNTS,
    newAccounts:[wallet.pubkey,recipient.pubkey,clone.pubkey,ownProgram.pubkey,owned.pubkey]});
const {boundary,datasource}=built,original=boundary.expected.original,tx=boundary.transactionBase64;
const image=key=>{const a=datasource.state.get(key);assert.ok(a!==undefined,`${key} is not served`);return a;};
const {keys,instructions}=parse(tx,base58Encode),payer=keys[0];
const before=key=>original.accountTransitions.find(t=>t.pubkey===key).before;
const token=original.accountTransitions.find(t=>t.before.tokenAmount!==null);  // a token account the transaction writes
const mint=base58Encode(Buffer.from(image(token.pubkey).dataBase64,"base64").subarray(0,32)),tokenProgram=image(mint).owner;
const owner=base58Encode(Buffer.from(image(token.pubkey).dataBase64,"base64").subarray(32,64));
const program=instructions.map(i=>i.program).find(p=>image(p).owner===UPGRADEABLE&&!TOKEN_PROGRAMS.includes(p));
const walletAta=findProgramAddress([wallet.pubkey,tokenProgram,mint].map(base58Decode),ATA);
const historicalTime=clockTime(boundary.clock.dataBase64),historicalSlot=clockSlot(boundary.clock.dataBase64);
// Historical accounts for the flows that edit an account Surfpool has not read yet.
const served=[...datasource.state.values()],SYSTEM="11111111111111111111111111111111";
const tokenFields=a=>{const d=Buffer.from(a.dataBase64,"base64");return {mint:base58Encode(d.subarray(0,32)),owner:base58Encode(d.subarray(32,64))};};
const isTokenAccount=a=>{const d=Buffer.from(a.dataBase64,"base64");return TOKEN_PROGRAMS.includes(a.owner)&&(d.length===165||(d.length>165&&d[165]===2));};
const historicalWallet=served.find(a=>a.owner===SYSTEM&&a.dataBase64===""&&a.pubkey!==payer&&BigInt(a.lamports)>0n);
const historicalAta=served.find(a=>isTokenAccount(a)&&a.pubkey===findProgramAddress([tokenFields(a).owner,a.owner,tokenFields(a).mint].map(base58Decode),ATA));
const historicalToken=served.find(a=>isTokenAccount(a)&&a.pubkey!==historicalAta?.pubkey);
const programDataOfTarget=image(programDataAddress(program));
const donor=served.filter(a=>a.owner===UPGRADEABLE&&a.pubkey!==programDataOfTarget.pubkey&&Buffer.from(a.dataBase64,"base64").readUInt32LE(0)===3)
    .map(a=>Buffer.from(a.dataBase64,"base64").subarray(45)).filter(elf=>elf.length<=Buffer.from(programDataOfTarget.dataBase64,"base64").length-45)
    .sort((a,b)=>a.length-b.length)[0];
// Programs built today are sBPF v3; mainnet runs them only from the activation of enable_sbpf_v3_deployment_and_execution.
const SBPF_V3="5cC3foj77CWun58pC51ebHFUWavHWKarWyR5UUik7dnC",runsV3=(!matchDatasource||boundary.features.some(f=>f.id===SBPF_V3));
const declaredProgramData=declareProgramData(built,programDataAddress);
let surfpool;
const start=async()=>{surfpool=await startSurfpool({datasource:built.datasource.url,matchDatasource,binary:surfpoolBinary,blockMode:"transaction"});};
await start();
const call=(method,params=[])=>surfpool.call(method,params);
const account=async key=>(await call("getAccountInfo",[key,{encoding:"base64",commitment:"processed"}])).value;
const balance=async key=>String((await call("getBalance",[key,{commitment:"processed"}])).value);
const landed=async sig=>{for(let i=0;i<40;i++){const {value}=await call("getSignatureStatuses",[[sig]]);if(value[0]!==null)return value[0];
    await new Promise(r=>setTimeout(r,250));}throw new Error(`${sig} did not land`);};
const originalGrade=async()=>grade(await simulate(surfpool,tx),original);
// A key the fork refused must be named in its status, so the developer knows what to declare (Surfpool shows "Internal error").
const refused=(key,run)=>async()=>{
    try {await run();} catch(error){
        if(error?.code!=="SURFPOOL_RPC")throw error;
        assert.ok(datasource.misses.has(key),`${key} is not named in svmReplayFork_status`);
        return {branch:"explicit",detail:`${error.message.slice(0,160)} | named in svmReplayFork_status`};
    }
    return {branch:"silent",detail:"succeeded although the fork never served this undeclared account"};
};
const both=(...branches)=>({stock:branches,match:branches});

const flows=[
 ["requestAirdrop to a historical wallet Surfpool has not read adds to its historical balance",both("silent"),async()=>{
     await landed(await call("requestAirdrop",[historicalWallet.pubkey,Number(SOL)]));
     assert.equal(await balance(historicalWallet.pubkey),String(BigInt(historicalWallet.lamports)+SOL),`historical balance ${historicalWallet.lamports}`);}],
 ["requestAirdrop after reading the wallet adds to its historical balance (documented workaround)",both("ok"),async()=>{
     await call("surfnet_resetAccount",[historicalWallet.pubkey,{includeOwnedAccounts:false}]);
     assert.equal(await balance(historicalWallet.pubkey),historicalWallet.lamports,"resetAccount did not restore the history");
     await landed(await call("requestAirdrop",[historicalWallet.pubkey,Number(SOL)]));
     assert.equal(await balance(historicalWallet.pubkey),String(BigInt(historicalWallet.lamports)+SOL));}],
 ["setAccount (lamports only) on a historical token account Surfpool has not read keeps its data",both("ok"),async()=>{
     await call("surfnet_setAccount",[historicalToken.pubkey,{lamports:Number(BigInt(historicalToken.lamports)+1n)}]);
     const seen=await account(historicalToken.pubkey);assert.equal(seen.owner,historicalToken.owner);
     assert.equal(seen.data[0],historicalToken.dataBase64,"token data was replaced");}],
 ["setTokenAccount (amount only) on a historical associated token account keeps its other fields",both("ok","n/a"),async()=>{
     if(historicalAta===undefined)return {branch:"n/a",detail:"no historical associated token account in this transaction"};
     const {owner:holder,mint:ataMint}=tokenFields(historicalAta);
     await call("surfnet_setTokenAccount",[holder,ataMint,{amount:777},historicalAta.owner]);
     const seen=await account(historicalAta.pubkey),data=Buffer.from(seen.data[0],"base64"),history=Buffer.from(historicalAta.dataBase64,"base64");
     assert.equal(data.readBigUInt64LE(64),777n,"amount not set");data.writeBigUInt64LE(history.readBigUInt64LE(64),64);
     assert.equal(String(seen.lamports),historicalAta.lamports,"lamports changed");assert.ok(data.equals(history),"fields other than the amount changed");}],
 ["requestAirdrop to an undeclared fresh key",both("ok"),async()=>{
     await landed(await call("requestAirdrop",[stranger.pubkey,Number(SOL)]));
     return {branch:"ok",detail:"correct for a fresh key, but unchecked: Surfpool's airdrop reads only its local state, never the fork"};}],
 ["(Surfpool restarted) simulateTransaction of the original = its verified result",{stock:["n/a"],match:["ok"]},async()=>{
     await surfpool.stop();await start();if(matchDatasource)assert.equal(await originalGrade(),"G2");
     else return {branch:"n/a",detail:"stock keeps today's Clock and bundled programs; historical parity not asserted"};}],
 ["profileTransaction of the original = its verified compute units and logs",{stock:["n/a"],match:["ok"]},async()=>{
     if(!matchDatasource)return {branch:"n/a",detail:"stock runtime is not historical"};
     const profile=(await call("surfnet_profileTransaction",[tx,"flows"])).value.transactionProfile,logs=profile.logMessages??[];
     const at=logs.findIndex((line,i)=>line!==original.logs[i]);
     assert.ok(profile.computeUnitsConsumed===original.computeUnits&&at===-1&&logs.length===original.logs.length,
         `units ${profile.computeUnitsConsumed} vs ${original.computeUnits}; ${logs.length} vs ${original.logs.length} log lines, first difference `+
         `at ${at}: ${JSON.stringify(logs[at])} vs ${JSON.stringify(original.logs[at])}`);}],
 ["getBalance of the fee payer = its verified balance before the transaction",both("ok"),async()=>{assert.equal(await balance(payer),before(payer).lamports);}],
 ["getTokenAccountBalance = the verified token amount before the transaction",both("ok"),async()=>{
     assert.equal((await call("getTokenAccountBalance",[token.pubkey,{commitment:"processed"}])).value.amount,token.before.tokenAmount);}],
 ["getAccountInfo jsonParsed decodes the historical token account",both("ok"),async()=>{
     const {value}=await call("getAccountInfo",[token.pubkey,{encoding:"jsonParsed",commitment:"processed"}]);
     assert.equal(value.data.parsed.info.tokenAmount.amount,token.before.tokenAmount);}],
 ["getTokenSupply of its mint = the historical supply",both("ok"),async()=>{
     assert.equal((await call("getTokenSupply",[mint,{commitment:"processed"}])).value.amount,mintSupply(image(mint).dataBase64));}],
 ["getMinimumBalanceForRentExemption(165) follows the historical Rent sysvar",{stock:["n/a"],match:["ok"]},async()=>{
     if(!matchDatasource)return {branch:"n/a",detail:"stock uses its bundled Rent"};
     assert.equal(String(await call("getMinimumBalanceForRentExemption",[165])),rentExemption(image(RENT).dataBase64,165));}],
 ["getGenesisHash = mainnet's",both("ok"),async()=>{assert.equal(await call("getGenesisHash"),MAINNET_GENESIS);}],
 ["getTransaction of the target before it runs = null",both("ok","explicit"),async()=>{
     assert.equal(await call("getTransaction",[boundary.signature,{maxSupportedTransactionVersion:0,encoding:"base64"}]),null);}],
 ["getProgramAccounts (a historical scan)",both("explicit"),async()=>{
     const value=await call("getProgramAccounts",[tokenProgram,{encoding:"base64",dataSlice:{offset:0,length:0},filters:[{memcmp:{offset:0,bytes:mint}}]}]);
     return {branch:"silent",detail:`${value.length} accounts, not the historical set`};}],
 ["getTokenAccountsByOwner (a historical scan)",both("explicit"),async()=>{
     const {value}=await call("getTokenAccountsByOwner",[owner,{mint},{encoding:"base64"}]);
     return {branch:"silent",detail:`${value.length} accounts, not the historical set`};}],
 ["getSignaturesForAddress of the fee payer (history)",both("explicit"),async()=>{
     const value=await call("getSignaturesForAddress",[payer,{limit:5}]);return {branch:"silent",detail:`${value.length} signatures, not its history`};}],
 ["requestAirdrop to a declared new wallet",both("ok"),async()=>{
     await landed(await call("requestAirdrop",[wallet.pubkey,Number(10n*SOL)]));assert.equal(await balance(wallet.pubkey),String(10n*SOL));}],
 ["sendTransaction: a signed transfer to a declared new key, then getTransaction of it",both("ok"),async()=>{
     const {blockhash}=(await call("getLatestBlockhash",[{commitment:"processed"}])).value;
     const sig=await call("sendTransaction",[transfer(wallet,recipient.pubkey,SOL,blockhash,base58Decode),{encoding:"base64"}]);
     await landed(sig);assert.equal(await balance(recipient.pubkey),String(SOL));
     assert.notEqual(await call("getTransaction",[sig,{maxSupportedTransactionVersion:0,commitment:"confirmed"}]),null);}],
 ["setTokenAccount before its token account is declared",both("explicit"),refused(walletAta,async()=>{
     await call("surfnet_setTokenAccount",[wallet.pubkey,mint,{amount:123456},tokenProgram]);})],
 ["setTokenAccount after declaring it (fork.datasource.fresh)",both("ok"),async()=>{
     datasource.fresh.add(walletAta);await call("surfnet_setTokenAccount",[wallet.pubkey,mint,{amount:123456},tokenProgram]);
     assert.equal((await call("getTokenAccountBalance",[walletAta,{commitment:"processed"}])).value.amount,"123456");}],
 ["getTokenAccountsByOwner of a declared new wallet lists the token account created on the fork",both("ok"),async()=>{
     const {value}=await call("getTokenAccountsByOwner",[wallet.pubkey,{mint},{encoding:"base64",commitment:"processed"}]);
     assert.deepEqual(value.map(a=>a.pubkey),[walletAta]);}],
 ["getProgramAccounts of a program declared new lists what was created on the fork (Anchor's .all())",both("ok"),async()=>{
     await call("surfnet_setAccount",[owned.pubkey,{lamports:1_000_000,owner:ownProgram.pubkey,data:"0102030405060708"}]);
     const value=await call("getProgramAccounts",[ownProgram.pubkey,{encoding:"base64",commitment:"processed"}]);
     assert.deepEqual(value.map(a=>a.pubkey),[owned.pubkey]);}],
 ["setAccount on a historical account; resetAccount restores it",both("ok"),async()=>{
     await call("surfnet_setAccount",[payer,{lamports:42}]);assert.equal(await balance(payer),"42");
     await call("surfnet_resetAccount",[payer,{includeOwnedAccounts:false}]);assert.equal(await balance(payer),before(payer).lamports);}],
 ["writeProgram with code the historical runtime loads: it runs; resetAccount restores the original",both("ok","n/a"),async()=>{
     if(donor===undefined)return {branch:"n/a",detail:"no other upgradeable program small enough in this transaction"};
     const unpatched=await simulate(surfpool,tx);
     try {
         await call("surfnet_writeProgram",[program,donor.toString("hex"),0]);
         const patched=await simulate(surfpool,tx);
         assert.notDeepEqual([patched.err,patched.logs],[unpatched.err,unpatched.logs],"the patched program did not run");
     } finally {for(const key of [program,programDataAddress(program)])await call("surfnet_resetAccount",[key,{includeOwnedAccounts:false}]);}
     const restored=await simulate(surfpool,tx);assert.deepEqual([restored.err,restored.logs],[unpatched.err,unpatched.logs],"reset did not restore it");}],
 [`writeProgram with a program built today (sBPF v3): ${runsV3?"runs (v3 active)":"Surfpool keeps the old program (v3 not active yet)"}`,
     both(runsV3?"ok":"silent"),async()=>{
     const unpatched=await simulate(surfpool,tx);
     try {
         await call("surfnet_writeProgram",[program,(await readFile(programFile)).toString("hex"),0]);
         const patched=await simulate(surfpool,tx);
         assert.notDeepEqual([patched.err,patched.logs],[unpatched.err,unpatched.logs],"Surfpool reported success but kept running the previous program");
     } finally {for(const key of [program,programDataAddress(program)])await call("surfnet_resetAccount",[key,{includeOwnedAccounts:false}]);}
     return {branch:"ok",detail:"the new program ran"};}],
 ["setProgramAuthority on a historical program",both("ok"),async()=>{
     await call("surfnet_setProgramAuthority",[program,wallet.pubkey]);
     assert.equal(upgradeAuthority((await account(programDataAddress(program))).data[0],base58Encode),wallet.pubkey);}],
 ["cloneProgramAccount to a declared new address",both("ok"),async()=>{
     datasource.fresh.add(programDataAddress(clone.pubkey));await call("surfnet_cloneProgramAccount",[program,clone.pubkey]);
     assert.equal((await account(clone.pubkey)).executable,true);}],
 ["the Clock after new blocks stays at the historical moment (plus elapsed time)",{stock:["silent"],match:["ok"]},async()=>{
     // The fork's own elapsed time: Surfpool adds its slot time (400 ms at most by default) per block, and transaction mode
     // produces blocks until each transaction is finalized (Surfpool 1.6.1), so it can run ahead of the wall clock.
     const data=(await account(CLOCK)).data[0],time=clockTime(data),slots=clockSlot(data)-historicalSlot;
     assert.ok(time>=historicalTime&&time<=historicalTime+slots*400n/1000n+5n,`clock ${time}, historical ${historicalTime}, ${slots} slots later`);}],
 ["timeTravel forward from the historical moment",{stock:["explicit"],match:["ok"]},async()=>{
     await call("surfnet_timeTravel",[{absoluteTimestamp:Number(historicalTime+86_400n)*1000}]);
     const time=clockTime((await account(CLOCK)).data[0]);assert.ok(time>=historicalTime+86_400n&&time<=historicalTime+86_460n,`clock ${time}`);}],
 ["timeTravel to before the historical moment",both("explicit"),async()=>{
     await call("surfnet_timeTravel",[{absoluteTimestamp:Number(historicalTime-86_400n)*1000}]);
     return {branch:"silent",detail:`clock now ${clockTime((await account(CLOCK)).data[0])}, before the fork's own history`};}],
 ["pauseClock and resumeClock",both("ok"),async()=>{await call("surfnet_pauseClock");await call("surfnet_resumeClock");}],
 ["exportSnapshot includes the historical accounts read",both("ok"),async()=>{
     const snapshot=JSON.stringify(await call("surfnet_exportSnapshot",[]));assert.ok(snapshot.includes(payer)&&snapshot.includes(token.pubkey));}],
 ["resetNetwork restores the historical fee payer",both("ok"),async()=>{
     await call("surfnet_resetNetwork");assert.equal(await balance(payer),before(payer).lamports);}],
 ["after resetNetwork the matched runtime reaches the verified grade",{stock:["n/a"],match:["ok"]},async()=>{
     if(!matchDatasource)return {branch:"n/a",detail:"stock uses today's Clock and bundled programs"};
     assert.equal(await originalGrade(),"G2");}],
];
const rows=[];
try {
for(const [flow,expect,run] of flows){
    const result=await attempt(run),pass=expect[mode].includes(result.branch);
    rows.push({flow,expected:expect[mode],...result,pass});console.log(JSON.stringify(rows.at(-1)));
}
} finally {await surfpool.stop();await built.close();}
const failed=rows.filter(r=>!r.pass),status=failed.length===0?"PASS":"FAIL";
await save("summary.json",{status,mode,signature,runtime:boundary.runtime?.name??null,program,mint,flows:rows});
console.log(JSON.stringify({status,mode,flows:rows.length,failed:failed.map(r=>r.flow)}));
process.exitCode=status==="PASS"?0:1;
