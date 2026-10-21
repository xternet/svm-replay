/**
 * Opt-in driver: Anchor's TypeScript client (@anchor-lang/core) with a mainnet Anchor program on a fork. Its on-chain IDL
 * is refused and named until the fork reads it (--also); then the client decodes the program's historical accounts, whose
 * bytes must be SVM Replay's verified state, and the target's instruction, while a scan of the program's accounts fails
 * explicitly. JS_DIR: where @anchor-lang/core is installed.
 * Usage: API_ALCHEMY=… node anchor-idl.mjs SDK_ENTRY INSTALLATION SURFPOOL JS_DIR DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE PROGRAM
 */
import assert from "node:assert/strict";
import {createRequire} from "node:module";
import {createHash} from "node:crypto";
import {inflateSync} from "node:zlib";
import {join,resolve} from "node:path";
import {startSurfpool,SUPPORT_ACCOUNTS,declareProgramData,open,output,parse,hash,exact} from "./shared.mjs";
const [entry,installation,surfpoolBinary,jsDir,dataDir,alchemyConfig,outArg,signature,programId]=process.argv.slice(2);
if(programId===undefined)throw new Error("usage: API_ALCHEMY=… node anchor-idl.mjs SDK_ENTRY INSTALLATION SURFPOOL JS_DIR DATA_DIR ALCHEMY_CONFIG NEW_OUT SIGNATURE PROGRAM");
const matchDatasource=process.env.SVM_REPLAY_FORK_MATCH_DATASOURCE==="1";
const require=createRequire(join(resolve(jsDir),"package.json"));
const anchor=require("@anchor-lang/core"),{Connection,Keypair,PublicKey}=require("@solana/web3.js");
const {fork,replay,base58Encode,base58Decode,findProgramAddress,programDataAddress,footprintTransaction}=await open(entry,installation,dataDir,alchemyConfig);
// Anchor's legacy IDL account: createWithSeed(the program's PDA of no seeds, "anchor:idl", program).
const legacyIdl=base58Encode(createHash("sha256").update(Buffer.concat([base58Decode(findProgramAddress([],programId)),
    Buffer.from("anchor:idl"),base58Decode(programId)])).digest());
const save=await output(outArg),checks={};let transactionBase64;
async function session(also,run,newAccounts=[]){
    const built=await fork.startHistorical(replay,{tx:signature,also:[...also,...SUPPORT_ACCOUNTS],newAccounts});
    declareProgramData(built,programDataAddress);
    const surfpool=await startSurfpool({datasource:built.datasource.url,matchDatasource,binary:surfpoolBinary,blockMode:"transaction"});
    try {
        const provider=new anchor.AnchorProvider(new Connection(surfpool.url,"processed"),new anchor.Wallet(Keypair.generate()),{commitment:"processed"});
        return await run(built,provider);
    } finally {await surfpool.stop();await built.close();}
}

// 1. The IDL account (Anchor 1.x reads the Program Metadata IDL) is outside the transaction's state: fetchIdl fails, and
// the fork names the account to add.
const named=await session([],async(built,provider)=>{
    transactionBase64=built.boundary.transactionBase64;
    const error=await anchor.Program.fetchIdl(new PublicKey(programId),provider).then(()=>null,e=>e);
    assert.ok(error!==null,"fetchIdl succeeded without the IDL account");
    const misses=[...built.datasource.misses];assert.equal(misses.length,1,`${error.message}; misses ${misses}`);
    assert.ok(error.message.includes(misses[0]),"the client's error names another account");checks.idlRefused=misses[0];return misses;
});
// 2. Restarted with --also for it, as the error says. If the account did not exist then, SVM Replay cannot prove that from
// the block and refuses; the developer declares it new (fetchIdl then answers null, as mainnet did) and reads the
// program's legacy Anchor IDL account instead.
let also=named,declared=[];
const widened=await exact(replay,signature,footprintTransaction(transactionBase64,[...also,...SUPPORT_ACCOUNTS])).then(()=>null,e=>e);
if(widened!==null){
    assert.equal(widened.details?.receipt?.error?.details?.reason,"unproven-account-absence",widened.message);
    checks.idlAccount="absent then: unprovable, declared new";also=[legacyIdl];declared=named;
}
await session(also,async(built,provider)=>{
    let idl=await anchor.Program.fetchIdl(new PublicKey(programId),provider);
    if(idl===null){
        assert.ok(declared.length>0,"fetchIdl found no IDL in an account the fork served");
        const data=(await provider.connection.getAccountInfo(new PublicKey(legacyIdl))).data;  // 8 discriminator, 32 authority, u32 length
        idl=JSON.parse(inflateSync(data.subarray(44,44+data.readUInt32LE(40))).toString("utf8"));checks.idlSource="legacy anchor:idl account";
    } else checks.idlSource="Program Metadata IDL";
    assert.ok(idl.address===programId,"the IDL is for another program");checks.idl={name:idl.metadata?.name,version:idl.metadata?.version,spec:idl.metadata?.spec};
    const program=new anchor.Program(idl,provider),original=built.boundary.expected.original;
    const camel=name=>name.charAt(0).toLowerCase()+name.slice(1);
    checks.accounts=[];
    for(const t of original.accountTransitions.filter(t=>t.before.presence==="present"&&t.before.owner===programId)){
        const info=await provider.connection.getAccountInfo(new PublicKey(t.pubkey));
        assert.equal(hash(info.data),t.before.dataHash,`${t.pubkey}: not the verified state`);
        const type=idl.accounts.find(a=>Buffer.from(a.discriminator).equals(info.data.subarray(0,8)));
        assert.ok(type!==undefined,`${t.pubkey}: no IDL account type`);
        const decoded=await program.account[camel(type.name)].fetch(t.pubkey);
        assert.ok(decoded!==null);checks.accounts.push({pubkey:t.pubkey,type:type.name,fields:Object.keys(decoded).length});
    }
    assert.ok(checks.accounts.length>0,"the transaction loads no account of this program");
    const {tx,instructions}=parse(built.boundary.transactionBase64,base58Encode),ix=instructions.find(i=>i.program===programId);
    const decoded=program.coder.instruction.decode(Buffer.from(tx.subarray(ix.at,ix.at+ix.length)));
    assert.ok(decoded!==null,"the target's instruction does not decode");checks.instruction=decoded.name;
    // A scan of a historical program's accounts has no historical source: an explicit error, never a partial list.
    await assert.rejects(program.account[camel(checks.accounts[0].type)].all());checks.scanRefused=true;
},declared);
await save("summary.json",{status:"PASS",matchDatasource,signature,programId,checks});
console.log(JSON.stringify({status:"PASS",matchDatasource,checks}));
