/**
 * Opt-in driver: several forks in one test process (Foundry's createSelectFork), each just before its own transaction,
 * all live at once. Each Surfpool must serve its own boundary and reach its own transaction's verified result, with no
 * state crossing between them.
 * Usage: API_ALCHEMY=… node forks.mjs SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE SIGNATURE...
 */
import assert from "node:assert/strict";
import {startSurfpool,SUPPORT_ACCOUNTS,declareProgramData,open,output,simulate,grade,parse} from "./shared.mjs";
const [entry,installation,surfpoolBinary,dataDir,alchemyConfig,outArg,...signatures]=process.argv.slice(2);
if(signatures.length<2)throw new Error("usage: API_ALCHEMY=… node forks.mjs SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE SIGNATURE...");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const {fork,replay,base58Encode,programDataAddress}=await open(entry,installation,dataDir,alchemyConfig);
const save=await output(outArg),forks=[];
try {
    for(const signature of signatures){
        const built=await fork.startHistorical(replay,{tx:signature,also:SUPPORT_ACCOUNTS});
        declareProgramData(built,programDataAddress);
        forks.push({signature,built,surfpool:await startSurfpool({datasource:built.datasource.url,matchDatasource,binary:surfpoolBinary,blockMode:"transaction"})});
    }
    const results=[];
    for(const {signature,built,surfpool} of forks){
        const original=built.boundary.expected.original,payer=parse(built.boundary.transactionBase64,base58Encode).keys[0];
        const before=original.accountTransitions.find(t=>t.pubkey===payer).before.lamports;
        assert.equal(String((await surfpool.call("getBalance",[payer,{commitment:"processed"}])).value),before,`${signature}: fee payer`);
        assert.equal(await surfpool.call("getSlot",[{commitment:"processed"}]),built.boundary.targetSlot,`${signature}: slot`);
        if(matchDatasource)assert.equal(grade(await simulate(surfpool,built.boundary.transactionBase64),original),"G2",`${signature}: original`);
        // Each fee payer, read on every other fork, is that fork's answer (its own history or an explicit refusal).
        for(const other of forks.filter(f=>f.signature!==signature)){
            const seen=await other.surfpool.call("getBalance",[payer,{commitment:"processed"}]).then(r=>String(r.value),()=>"refused");
            const own=other.built.boundary.accounts.get(payer);
            assert.equal(seen,own===undefined?"refused":own.lamports,`${signature}'s payer read on ${other.signature}`);
        }
        results.push({signature,runtime:built.boundary.runtime?.name??null,slot:built.boundary.targetSlot,grade:matchDatasource?"G2":null,limitation:matchDatasource?null:"stock Clock and bundled programs are current"});
    }
    await save("summary.json",{status:"PASS",matchDatasource,forks:results});
    console.log(JSON.stringify({status:"PASS",matchDatasource,forks:results}));
} finally {for(const {built,surfpool} of forks){await surfpool.stop();await built.close();}}
