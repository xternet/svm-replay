/**
 * Opt-in historical driver: Surfpool forked just before real mainnet transactions (svm-replay fork).
 * Expectations come from SVM Replay's evidence, read here from each job's receipt, never from the fork's served state.
 * Targets are SIGNATURE or SIGNATURE:GRADE (the grade the original must reach, G0..G2).
 * Set SVM_REPLAY_FORK_MATCH_DATASOURCE=1 for a Surfpool build with --match-datasource.
 */
import assert from "node:assert/strict";
import {startSurfpool,SUPPORT_ACCOUNTS,declareProgramData,open,output,state,sameState,accounts,simulate,grade,hash,evidence,expectedBoundary,effects,parse,lowerComputeLimit,flipLastData,exact,FIELDS}
    from "./shared.mjs";
const [entry,installation,surfpoolBinary,dataDir,alchemyConfig,outArg,...targets]=process.argv.slice(2);
if(targets.length===0)throw new Error("usage: API_ALCHEMY=… node surfpool.mjs INSTALLED_SDK_ENTRY INSTALLATION SURFPOOL DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE[:GRADE]...");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const {fork,replay,loadClosedBoundary,base58Encode,base58Decode,programDataAddress}=await open(entry,installation,dataDir,alchemyConfig);
const save=await output(outArg);
const DECLARED="9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin",UNDECLARED="BRzqH3n8jxDm7awtNnLq2sQJuT7kTn5wYQhSBP4LcEK4";
const CLOCK="SysvarC1ock11111111111111111111111111111111",TOKEN="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const NATIVE="NativeLoader1111111111111111111111111111111";
// Surfpool's own SlotHashes and RecentBlockhashes match the blockhashes it produces; they are not served history.
const OWN_SYSVARS=new Set(["SysvarS1otHashes111111111111111111111111111","SysvarRecentB1ockHashes11111111111111111111"]);
const results=[];let incidentalChecked=false;
const outcome=r=>JSON.stringify([r.status,r.normalizedError??null,r.logs,r.computeUnits]);

for(const target of targets){
    const [signature,expectedGrade]=target.split(":");
    const started=performance.now(),checks={};
    const built=await fork.startHistorical(replay,{tx:signature,also:SUPPORT_ACCOUNTS,newAccounts:[DECLARED]});
    const {boundary,datasource}=built,original=boundary.expected.original;
    const jobs=await Promise.all(boundary.evidence.map(evidence));
    const previousBlockhash=JSON.parse(Buffer.from(jobs[0].prepared.rawBlockBase64,"base64")).result.previousBlockhash;
    checks.declaredProgramData=declareProgramData(built,programDataAddress);
    const surfpool=await startSurfpool({datasource:built.datasource.url,matchDatasource,binary:surfpoolBinary,blockMode:"manual"}).catch(async error=>{console.error(JSON.stringify({startupMisses:[...datasource.misses],signature}));await built.close();throw error;});
    try {
        if(!matchDatasource){
            const bundled=new Set([...SUPPORT_ACCOUNTS,...SUPPORT_ACCOUNTS.map(programDataAddress)]);
            const keys=[...datasource.state.keys()].filter(key=>!bundled.has(key)&&
                datasource.state.get(key).owner!==NATIVE&&datasource.state.get(key).owner!=="Sysvar1111111111111111111111111111111111111");
            const seen=await accounts(surfpool,keys);
            keys.forEach((key,i)=>sameState(state(seen[i]),state({...datasource.state.get(key),data:[datasource.state.get(key).dataBase64]}),`stock historical read ${key}`));
            assert.ok(keys.length>0,"no historical application state checked");
            const [clock]=await accounts(surfpool,[CLOCK]);
            checks.historicalReads=keys.length;checks.stock={bundledAccounts:[...bundled].filter(key=>datasource.state.has(key)),clockData:clock.data[0],historicalClockData:boundary.clock.dataBase64,
                limitation:"Stock Surfpool uses today's Clock and bundled programs; execution parity is not asserted."};
            results.push({signature,family:boundary.family,checks,ms:Math.round(performance.now()-started)});
            console.log(JSON.stringify(results.at(-1)));continue;
        }
        // 1. The fork starts at the target's slot and epoch, with the verified historical feature set (nothing kept).
        assert.equal(await surfpool.call("getSlot",[{commitment:"processed"}]),boundary.targetSlot);
        assert.equal((await surfpool.call("getEpochInfo",[])).epoch,boundary.epoch);
        checks.start=true;

        // 2. Every served account, read through Surfpool before anything runs, equals the state derived independently from
        // the evidence of the job that proved it. Built-in programs are Surfpool's own copies: rentEpoch 0, history u64::MAX.
        const expected=expectedBoundary(jobs[0]);
        for(const job of jobs.slice(1)){
            const derived=expectedBoundary(job);
            for(const key of parse(job.prepared.fixture.target.replacementTransactionBase64,base58Encode).keys.slice(1)){
                expected.states.set(key,derived.states.get(key));if(derived.changed.has(key))expected.changed.add(key);
            }
        }
        const keys=[...datasource.state.keys()].filter(k=>k!==CLOCK&&!OWN_SYSVARS.has(k));
        const seen=await accounts(surfpool,keys),builtins=[];let writtenEarlier=0;
        keys.forEach((key,i)=>{
            const want=expected.states.get(key),got=state(seen[i]);
            assert.ok(want!==undefined,`${key} is served without evidence`);
            if(datasource.state.get(key).owner===NATIVE){
                const fields=FIELDS.filter(f=>String(got[f])!==String(want[f]));
                assert.ok(fields.every(f=>f==="rentEpoch"),`built-in ${key} differs in ${fields}`);
                if(fields.length>0){assert.deepEqual([got.rentEpoch,want.rentEpoch],["0","18446744073709551615"],`built-in ${key}`);builtins.push(key);}
                return;
            }
            sameState(got,want,`boundary ${key}`);if(expected.changed.has(key))writtenEarlier++;
        });
        for(const key of boundary.unavailable)assert.ok(expected.changed.has(key),`${key} refused without a recorded same-slot write`);
        checks.boundary={served:keys.length,writtenEarlierInSlot:writtenEarlier,unavailable:boundary.unavailable.size,builtinRentEpochOnly:builtins.length};

        // 3. Matched Clock bytes equal the verified runtime pin; the local sysvar account holds 1 lamport.
        const [clock]=await accounts(surfpool,[CLOCK]),historicalClock=jobs[0].prepared.fixture.clock;
        const pin=jobs[0].prepared.fixture.runtime.genericSysvars.find(s=>s.pubkey===CLOCK);
        assert.equal(hash(Buffer.from(clock.data[0],"base64")),pin.dataSha256,"Clock bytes");assert.equal(clock.owner,historicalClock.owner);
        assert.equal(String(clock.lamports),"1","Clock lamports");
        checks.clock={lamports:String(clock.lamports),historicalLamports:historicalClock.lamports};

        // 4. The original transaction: exact status and error, logs, compute units and every account effect.
        const written=original.accountTransitions.map(t=>t.pubkey);
        const simulation=await simulate(surfpool,boundary.transactionBase64,written);
        checks.grade=grade(simulation,original);
        assert.equal(checks.grade,expectedGrade??checks.grade,"recorded grade");assert.notEqual(checks.grade,"none","original status must match");
        if(boundary.family==="v4-2")assert.equal(checks.grade,"G2","Surfpool's own runtime family must match compute units");
        checks.effects=effects(simulation,original.accountTransitions,previousBlockhash,base58Decode);

        // 5. Editing historical state changes the outcome to a fee failure; resetAccount restores the exact original outcome.
        const payer=parse(boundary.transactionBase64,base58Encode).keys[0];
        // 1 lamport, not 0: Surfpool deletes a zero-lamport account and would re-read history on the next access.
        await surfpool.call("surfnet_setAccount",[payer,{lamports:1}]);
        const poor=await simulate(surfpool,boundary.transactionBase64);
        assert.match(JSON.stringify(poor.err),/InsufficientFunds/,"a fee payer without funds must fail on fees");
        await surfpool.call("surfnet_resetAccount",[payer,{includeOwnedAccounts:false}]);
        const again=await simulate(surfpool,boundary.transactionBase64,written);
        assert.equal(grade(again,original),checks.grade,"reset restores history");
        assert.deepEqual(effects(again,original.accountTransitions,previousBlockhash,base58Decode),checks.effects);
        checks.editAndReset=poor.err;

        // 6. Unknown accounts fail explicitly through Surfpool; declared new accounts are empty.
        assert.equal((await surfpool.call("getAccountInfo",[DECLARED,{encoding:"base64"}])).value,null);
        await assert.rejects(surfpool.call("getAccountInfo",[UNDECLARED,{encoding:"base64"}]),undefined,"undeclared account must not be silently empty");
        assert.ok(datasource.misses.has(UNDECLARED));checks.unknownAccounts=true;

        // 7. Historical scans are not available in this version: an explicit error, never a silent local-only list.
        await assert.rejects(surfpool.call("getProgramAccounts",[TOKEN,{encoding:"base64",dataSlice:{offset:0,length:0}}]),undefined,"scan must fail loudly");
        checks.scansFailLoudly=true;

        // 8. SVM Replay's --replace reproduces the verified original, and agrees with Surfpool on a modified payload.
        // A replacement run lists every account it loaded; the original lists the ones it wrote.
        const same=await exact(replay,signature,boundary.transactionBase64);
        assert.equal(outcome(same),outcome(original));
        const byKey=new Map(same.accountTransitions.map(t=>[t.pubkey,t]));
        for(const t of original.accountTransitions)assert.deepEqual(byKey.get(t.pubkey),t,`${t.pubkey} in the exact check`);
        for(const t of same.accountTransitions)if(!written.includes(t.pubkey))assert.deepEqual(t.after,t.before,`${t.pubkey} is only loaded`);
        // Built-in programs are Surfpool's own copies (rentEpoch differs); every other loaded account is compared.
        const effectsOf=transitions=>transitions.filter(t=>t.before.owner!==NATIVE);
        const lowered=lowerComputeLimit(boundary.transactionBase64,original.computeUnits-1,base58Encode);
        const modified=lowered??flipLastData(boundary.transactionBase64,base58Encode);
        const replaced=await exact(replay,signature,modified),changes=effectsOf(replaced.accountTransitions);
        const local=await simulate(surfpool,modified,changes.map(t=>t.pubkey));
        assert.notEqual(outcome(replaced),outcome(original),"the modification must change the result");
        const modifiedGrade=grade(local,replaced);assert.notEqual(modifiedGrade,"none","Surfpool and the exact runtime must agree");
        if(boundary.family==="v4-2")assert.equal(modifiedGrade,"G2");
        checks.modified={kind:lowered===undefined?"last data byte flipped":"compute limit lowered",status:replaced.status,error:replaced.normalizedError,grade:modifiedGrade,
            effects:effects(local,changes,previousBlockhash,base58Decode)};

        // 9. Accounts of earlier same-slot transactions that the target does not load (once): requested with --also, each
        // is its exact state before the target (after its earlier writers) or refused; the target's own accounts are unchanged.
        const known=k=>boundary.accounts.has(k)||boundary.absent.has(k)||boundary.unavailable.has(k);
        const touched=[...expected.changed].filter(k=>!known(k)).slice(0,3);
        const untouched=jobs[0].prepared.fixture.accounts.find(a=>!known(a.pubkey)&&!expected.changed.has(a.pubkey));
        if(!incidentalChecked&&touched.length>0&&untouched!==undefined){
            const extra=[...touched,untouched.pubkey],wider=await loadClosedBoundary(replay,{tx:signature},extra);
            const job=expectedBoundary(await evidence(wider.evidence.at(-1)));
            for(const key of extra){
                if(wider.unavailable.has(key)){assert.ok(job.changed.has(key),`${key} refused without a recorded write`);continue;}
                const a=wider.accounts.get(key),seen=a===undefined?{presence:"absent"}:state({...a,data:[a.dataBase64]});
                sameState(seen,job.states.get(key),`--also ${key}`);
            }
            const servedExactly=touched.filter(k=>!wider.unavailable.has(k));
            assert.ok(servedExactly.length>0,"a same-slot write must be served exactly from the engine snapshot");
            for(const [key,a] of boundary.accounts)assert.deepEqual(wider.accounts.get(key),a,`${key} unchanged by --also`);
            checks.incidental={writtenEarlierExact:servedExactly.length,writtenEarlierRefused:touched.length-servedExactly.length,untouched:untouched.pubkey};
            incidentalChecked=true;
        }
        // 10. A failed transaction still pays its fee: sent for real, its persistent effects equal the verified after-states.
        if(original.status!=="ok"){
            const sent=await surfpool.call("sendTransaction",[boundary.transactionBase64,{encoding:"base64",skipPreflight:true}]);
            for(let i=0;i<40;i++){const {value}=await surfpool.call("getSignatureStatuses",[[sent]]);if(value[0]!==null)break;await new Promise(r=>setTimeout(r,250));}
            const after=await accounts(surfpool,written);
            original.accountTransitions.forEach((t,i)=>sameState(state(after[i]),t.after,`${t.pubkey} after the failed transaction`));
            checks.failedPersistentEffects=written.length;
        }
    } finally {await surfpool.stop();await built.close();}
    results.push({signature,family:boundary.family,predecessors:built.predecessors,evidenceJobs:boundary.evidence.length,
        features:boundary.features.length,misses:[...datasource.misses],derivedFeatures:[...datasource.derivedFeatures.keys()],checks,ms:Math.round(performance.now()-started)});
    console.log(JSON.stringify(results.at(-1)));
}
if(matchDatasource)assert.ok(incidentalChecked,"at least one target must have earlier same-slot accounts outside its footprint");
await save("summary.json",{status:"PASS",matchDatasource,surfpool:surfpoolBinary,results});
