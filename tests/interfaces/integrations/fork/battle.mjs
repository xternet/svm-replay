/**
 * Opt-in driver: the worst cases, compared exactly between the fork and SVM Replay's historical runtime.
 * A new program (battle/, built for sBPF v0) replaces the code of an existing mainnet program (HOST) in both runtimes:
 * on Surfpool through surfnet_setAccount, in SVM Replay as an account override (--replace with --overrides). Transactions composed around a real
 * one (through its own lookup tables) then call it: after the original's protocol call; calling it with changed amounts;
 * moving tokens and lamports to accounts the original never touched; creating its own account; re-entering itself to
 * the maximum depth and one level past it; and failing after calls. Each must reach the same status, error, logs and
 * account effects on both runtimes; equal compute units are reported (G2).
 * The host, the accounts the original never touches and an address the block proves empty come from SVM Replay's
 * evidence of the slot (pickAccounts). Two answers differ on purpose: SVM Replay cannot prove that an address never
 * seen was empty (the fork runs it as declared new), and only Surfpool refuses a transaction over 1232 bytes, as mainnet
 * does (SVM Replay's --replace runs it).
 * Usage: API_ALCHEMY=… node battle.mjs SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE BATTLE_SO
 */
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {startSurfpool,SUPPORT_ACCOUNTS,declareProgramData,accounts,sameState,open,output,simulate,grade,state,evidence,exact,FIELDS} from "./shared.mjs";
import {tableAddresses,parseMessage,compileMessage,battle,systemTransfer,tokenTransfer,createAccount,pickAccounts} from "./battle-lib.mjs";
const [entry,installation,surfpoolBinary,dataDir,alchemyConfig,outArg,signature,battleSo]=process.argv.slice(2);
if(battleSo===undefined)throw new Error("usage: see the header of battle.mjs");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const {fork,replay,base58Encode,base58Decode,findProgramAddress,programDataAddress}=await open(entry,installation,dataDir,alchemyConfig);
const save=await output(outArg);
const BUILTIN=new Set(["ComputeBudget111111111111111111111111111111","11111111111111111111111111111111",
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"]);
// The battle accounts, from the evidence of the target's own verified job.
const scout=await fork.loadBoundary(replay,{tx:signature}),job=await evidence(scout.evidence[0]);
const called=new Set(parseMessage(scout.transactionBase64,base58Encode,new Map(job.prepared.fixture.accounts
    .filter(a=>a.owner==="AddressLookupTab1e1111111111111111111111111"&&a.presence==="present").map(a=>[a.pubkey,tableAddresses(a.dataBase64,base58Encode)])))
    .instructions.map(ix=>ix.program));
const {host,tokenAccount,wallet,wallet2,empty}=pickAccounts(job,called,base58Encode);
const hostData=programDataAddress(host),pda=findProgramAddress([Buffer.from("battle")],host);
const progress=phase=>console.error(JSON.stringify({phase,heapMiB:Math.round(process.memoryUsage().heapUsed/2**20),at:new Date().toISOString()}));
progress("start");

// The fork reads the host program and the never-touched accounts at the same boundary; the battle account is new.
const built=await fork.startHistorical(replay,{tx:signature,
    also:[host,tokenAccount,wallet,wallet2,empty,...SUPPORT_ACCOUNTS],newAccounts:[pda]});
progress("boundary served");
declareProgramData(built,programDataAddress);
const surfpool=await startSurfpool({datasource:built.datasource.url,matchDatasource,binary:surfpoolBinary,blockMode:"manual"});
progress("surfpool started");
const rows=[];
try {
    if(!matchDatasource){
        const keys=[hostData,tokenAccount],seen=await accounts(surfpool,keys);
        keys.forEach((key,i)=>sameState(state(seen[i]),state({...built.datasource.state.get(key),data:[built.datasource.state.get(key).dataBase64]}),`stock read ${key}`));
        await save("summary.json",{status:"PASS",matchDatasource,historicalReads:keys.length,
            limitation:"Stock Surfpool keeps today's Clock and bundled programs; battle execution parity requires --match-datasource."});
        console.log(JSON.stringify({status:"PASS",matchDatasource,historicalReads:keys.length}));
    } else {
    // The same code in both runtimes: the host's ProgramData header (slot, authority) followed by the battle ELF.
    const historical=Buffer.from(built.datasource.state.get(hostData).dataBase64,"base64");
    const code=Buffer.concat([historical.subarray(0,45),await readFile(battleSo)]);
    await surfpool.call("surfnet_setAccount",[hostData,{data:code.toString("hex")}]);
    await surfpool.call("surfnet_setAccount",[host,{lamports:Number(built.datasource.state.get(host).lamports)}]);  // reloads it
    const overrides=[{pubkey:hostData,dataBase64:code.toString("base64")}];

    // The original's lookup tables, as the fork serves them: composed transactions load accounts through them too.
    const tables=new Map([...built.datasource.state.values()].filter(a=>a.owner==="AddressLookupTab1e1111111111111111111111111")
        .map(a=>[a.pubkey,tableAddresses(a.dataBase64,base58Encode)]));
    const t=parseMessage(built.boundary.transactionBase64,base58Encode,tables),user=t.payer,used=new Map(t.lookups.map(k=>[k,tables.get(k)]));
    const protocol=t.instructions.find(ix=>!BUILTIN.has(ix.program)&&ix.data.length>=24);
    assert.ok(protocol!==undefined,"the original calls no protocol with u64 arguments");
    const setup=t.instructions.slice(0,t.instructions.indexOf(protocol));
    // The wrapped-SOL account the original funds before its protocol call (the account of its SyncNative).
    const sync=setup.find(ix=>ix.program==="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"&&ix.data[0]===17);
    assert.ok(sync!==undefined,"the original wraps no SOL before its protocol call");
    const wrapped=sync.accounts[0].pubkey;
    const changed={...protocol,data:Buffer.from(protocol.data)};
    changed.data.writeBigUInt64LE(changed.data.readBigUInt64LE(8)/2n,8);changed.data.writeBigUInt64LE(0n,16);
    // Every battle instruction lists the host's ProgramData, so SVM Replay records the overridden account.
    const b=()=>{const ix=battle(host);ix.account(hostData);return ix;};
    const nest=(depth,inner)=>depth===0?inner:nest(depth-1,b().invoke(inner).build());

    const scenarios=[
        ["the battle program runs in both runtimes",[b().clock().log(user).build()]],
        ["after the original's protocol call, it reads what the call wrote",
            [...setup,protocol,b().log(wrapped).log(protocol.accounts[0].pubkey).clock().build()]],
        ["it calls the original's protocol with changed amounts",[...setup,b().invoke(changed).returnData().log(wrapped).build()]],
        ["it moves tokens and lamports to accounts the original never touched",[...setup,b().invoke(tokenTransfer(wrapped,tokenAccount,user,1_000_000))
            .invoke(systemTransfer(user,wallet,5_000)).log(tokenAccount).log(wallet).build()]],
        ["it creates an account at an address the block proves empty, writes it and moves lamports out",[b()
            .invoke(createAccount(user,empty,5_000_000,64,base58Decode(host))).write(empty,0,Buffer.from("historical battle")).move(empty,wallet2,1_500_000).log(empty).build()]],
        ["it creates its own account at a fresh address (exact check: absence unprovable)",[b().create(user,pda,64,Buffer.from("battle"))
            .invoke(systemTransfer(user,pda,2_000_000)).write(pda,0,Buffer.from("historical battle")).move(pda,wallet2,1_500_000).log(pda).build()],"exact refuses"],
        ["it fails after calling the protocol",[...setup,b().invoke(changed).returnData().fail(4242).build()]],
        // Call depth: the token transfer at stack height 5 (the classic limit), then at height 10 (past any limit).
        ["it re-enters itself to the deepest classic call depth",[...setup,nest(4,tokenTransfer(wrapped,tokenAccount,user,1_000))]],
        ["it re-enters itself past the call-depth limit",[...setup,nest(9,tokenTransfer(wrapped,tokenAccount,user,1_000))]],
        ["the whole original plus 700 operations: over the 1232-byte packet limit",[...t.instructions,
            Array.from({length:700}).reduce(ix=>ix.clock(),b()).build()],"fork refuses"],
    ];
    for(const [name,instructions,expect="both run"] of scenarios){
        const {transactionBase64,writable}=compileMessage(user,t.blockhash,instructions,base58Decode,used);
        const row={name,expect,bytes:Buffer.from(transactionBase64,"base64").length,writable:writable.length};
        progress(`exact: ${name}`);
        const historical=await exact(replay,signature,transactionBase64,overrides).then(r=>r,e=>({refused:String(e.message).slice(0,300)}));
        progress(`fork: ${name}`);
        const seen=await simulate(surfpool,transactionBase64,writable).then(r=>r,e=>({refused:String(e.message).slice(0,300)}));
        row.outcome=historical.refused!==undefined&&seen.refused!==undefined?"both refuse":historical.refused!==undefined?"exact refuses":
            seen.refused!==undefined?"fork refuses":"both run";
        if(historical.refused!==undefined)row.exactRefusal=historical.refused;
        if(seen.refused!==undefined)row.forkRefusal=seen.refused;
        if(row.outcome==="both run"){
            Object.assign(row,{status:historical.status,error:historical.normalizedError,grade:grade(seen,historical),units:[seen.unitsConsumed,historical.computeUnits]});
            if(historical.status==="ok"){
                const after=new Map(historical.accountTransitions.map(x=>[x.pubkey,x.after])),differences=[];
                writable.forEach((pubkey,i)=>{
                    const expected=after.get(pubkey),actual=state(seen.accounts[i]);
                    if(expected===undefined){differences.push(`${pubkey}: not in SVM Replay's transitions`);return;}
                    const fields=expected.presence==="absent"?["presence"]:FIELDS;
                    for(const k of fields)if(String(actual[k])!==String(expected[k]))differences.push(`${pubkey} ${k}: fork ${actual[k]}, exact ${expected[k]}`);
                });
                row.effects=differences.length===0?"equal":differences;
            }
            if(row.grade==="none"||row.grade==="G0")row.logs={fork:seen.logs,exact:historical.logs,forkError:seen.err};
        } else if(row.outcome==="exact refuses")row.fork={error:seen.err,logs:seen.logs?.slice(-3)};
        row.pass=row.outcome===expect&&(row.outcome!=="both run"||((row.grade==="G1"||row.grade==="G2")&&(row.status!=="ok"||row.effects==="equal")))&&
            (row.outcome!=="exact refuses"||seen.err===null);
        rows.push(row);console.log(JSON.stringify(row));
    }
    }
} finally {await surfpool.stop();await built.close();}
if(matchDatasource){
const pass=rows.every(r=>r.pass);
await save("summary.json",{status:pass?"PASS":"FAIL",matchDatasource,signature,accounts:{host,tokenAccount,wallet,wallet2,empty,pda},scenarios:rows});
console.log(JSON.stringify({status:pass?"PASS":"FAIL",matchDatasource,scenarios:rows.length,g2:rows.filter(r=>r.grade==="G2").length}));
process.exitCode=pass?0:1;

}
