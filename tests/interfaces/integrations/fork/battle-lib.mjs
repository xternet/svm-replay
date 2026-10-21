/** Helpers for the battle driver: transactions (legacy and v0: parse and compile) and the battle program's operations. */

/** The addresses of an address lookup table account (after its 56-byte header). */
export function tableAddresses(dataBase64,base58Encode){
    const data=Buffer.from(dataBase64,"base64"),out=[];
    for(let at=56;at+32<=data.length;at+=32)out.push(base58Encode(data.subarray(at,at+32)));
    return out;
}
/**
 * A transaction's message, legacy or v0: payer, blockhash, instructions with account metas, and the lookup tables it
 * uses. `tables` maps each table's address to its addresses (the historical table, from the fork's served state).
 */
export function parseMessage(transactionBase64,base58Encode,tables=new Map()){
    const tx=Buffer.from(transactionBase64,"base64");let at=0;
    const compact=()=>{let v=0;for(let shift=0;;shift+=7){const b=tx[at++];v|=(b&0x7f)<<shift;if((b&0x80)===0)return v;}};
    const signatures=compact();at+=64*signatures;  // not `at+=64*compact()`: `at` would be read before compact() moves it
    const v0=(tx[at]&0x80)!==0;
    if(v0){if((tx[at]&0x7f)!==0)throw new Error("unsupported message version");at+=1;}
    const [signed,readonlySigned,readonlyUnsigned]=[tx[at],tx[at+1],tx[at+2]];at+=3;
    const keys=[];for(let n=compact();n>0;n--){keys.push(base58Encode(tx.subarray(at,at+32)));at+=32;}
    const blockhash=Buffer.from(tx.subarray(at,at+32));at+=32;const raw=[];
    for(let n=compact();n>0;n--){
        const program=tx[at++],indexes=[];for(let m=compact();m>0;m--)indexes.push(tx[at++]);
        const length=compact();raw.push({program,indexes,data:Buffer.from(tx.subarray(at,at+length))});at+=length;
    }
    const lookups=[];
    for(let n=v0?compact():0;n>0;n--){
        const table=base58Encode(tx.subarray(at,at+32));at+=32;
        const writable=[];for(let m=compact();m>0;m--)writable.push(tx[at++]);
        const readonly=[];for(let m=compact();m>0;m--)readonly.push(tx[at++]);
        lookups.push({table,writable,readonly});
    }
    const resolve=(table,i)=>{const address=tables.get(table)?.[i];if(address===undefined)throw new Error(`lookup ${table}[${i}] is not available`);return address;};
    const loaded=[...lookups.flatMap(l=>l.writable.map(i=>resolve(l.table,i))),...lookups.flatMap(l=>l.readonly.map(i=>resolve(l.table,i)))];
    const loadedWritable=lookups.reduce((n,l)=>n+l.writable.length,0),all=[...keys,...loaded];
    const meta=i=>i<keys.length?{pubkey:keys[i],signer:i<signed,writable:i<signed?i<signed-readonlySigned:i<keys.length-readonlyUnsigned}
        :{pubkey:all[i],signer:false,writable:i<keys.length+loadedWritable};
    return {payer:keys[0],blockhash,lookups:lookups.map(l=>l.table),
        instructions:raw.map(r=>({program:all[r.program],accounts:r.indexes.map(meta),data:r.data}))};
}
const compactBytes=n=>{const out=[];do {let b=n&0x7f;n>>=7;if(n>0)b|=0x80;out.push(b);} while(n>0);return Buffer.from(out);};
/**
 * An unsigned transaction (signatures zero) for `instructions`, paid by `payer`, at `blockhash`. With `tables`
 * (address → addresses) it is v0: every account that is neither a signer nor an invoked program is loaded through the
 * first table holding it, with the privileges the instructions need. Without, it is legacy.
 */
export function compileMessage(payer,blockhash,instructions,base58Decode,tables=new Map()){
    const keys=new Map([[payer,{signer:true,writable:true,program:false}]]);
    const merge=(pubkey,signer,writable,program)=>{const k=keys.get(pubkey)??{signer:false,writable:false,program:false};
        keys.set(pubkey,{signer:k.signer||signer,writable:k.writable||writable,program:k.program||program});};
    for(const ix of instructions){for(const a of ix.accounts)merge(a.pubkey,a.signer,a.writable,false);merge(ix.program,false,false,true);}
    const where=new Map();for(const [table,addresses] of tables)addresses.forEach((a,index)=>{if(!where.has(a))where.set(a,{table,index});});
    const statics=[],loaded=[];
    for(const [pubkey,k] of keys){const w=where.get(pubkey);if(!k.signer&&!k.program&&w!==undefined)loaded.push({pubkey,...k,...w});else statics.push([pubkey,k]);}
    const rank=([pubkey,k])=>pubkey===payer?-1:(k.signer?0:2)+(k.writable?0:1);statics.sort((a,b)=>rank(a)-rank(b));
    const lookups=[...new Set(loaded.map(l=>l.table))].map(table=>({table,
        writable:loaded.filter(l=>l.table===table&&l.writable),readonly:loaded.filter(l=>l.table===table&&!l.writable)}));
    const order=[...statics.map(([pubkey])=>pubkey),...lookups.flatMap(l=>l.writable.map(x=>x.pubkey)),...lookups.flatMap(l=>l.readonly.map(x=>x.pubkey))];
    const index=new Map(order.map((pubkey,i)=>[pubkey,i])),signed=statics.filter(([,k])=>k.signer).length;
    const header=[signed,statics.filter(([,k])=>k.signer&&!k.writable).length,statics.filter(([,k])=>!k.signer&&!k.writable).length];
    const body=instructions.map(ix=>Buffer.concat([Buffer.from([index.get(ix.program)]),compactBytes(ix.accounts.length),
        Buffer.from(ix.accounts.map(a=>index.get(a.pubkey))),compactBytes(ix.data.length),ix.data]));
    const v0=tables.size>0,tail=!v0?[]:[compactBytes(lookups.length),...lookups.map(l=>Buffer.concat([base58Decode(l.table),
        compactBytes(l.writable.length),Buffer.from(l.writable.map(x=>x.index)),compactBytes(l.readonly.length),Buffer.from(l.readonly.map(x=>x.index))]))];
    const message=Buffer.concat([...(v0?[Buffer.from([0x80])]:[]),Buffer.from(header),compactBytes(statics.length),
        ...statics.map(([pubkey])=>base58Decode(pubkey)),blockhash,compactBytes(instructions.length),...body,...tail]);
    return {transactionBase64:Buffer.concat([compactBytes(signed),Buffer.alloc(64*signed),message]).toString("base64"),
        writable:[...statics.filter(([,k])=>k.writable).map(([pubkey])=>pubkey),...loaded.filter(l=>l.writable).map(l=>l.pubkey)]};
}

const u16=n=>{const b=Buffer.alloc(2);b.writeUInt16LE(n);return b;};
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32LE(n);return b;};
const u64=n=>{const b=Buffer.alloc(8);b.writeBigUInt64LE(BigInt(n));return b;};
/**
 * One battle instruction: `account(pubkey, {signer, writable})` returns the account's index in it, and the operation
 * builders append to its data. `build()` gives the instruction for `compileLegacy`.
 */
export function battle(program){
    const accounts=[],ops=[];
    const api={
        account(pubkey,{signer=false,writable=false}={}){
            let i=accounts.findIndex(a=>a.pubkey===pubkey);
            if(i<0){i=accounts.length;accounts.push({pubkey,signer,writable});}
            else accounts[i]={pubkey,signer:accounts[i].signer||signer,writable:accounts[i].writable||writable};
            if(i>255)throw new Error("a battle instruction indexes at most 256 accounts");return i;
        },
        /** Operation 1, or 2 when `signed` ({seed, address}) signs as this program's address: call `ix` ({program, accounts, data}). */
        invoke(ix,signed){
            const seed=signed?.seed;
            const metas=ix.accounts.map(a=>[api.account(a.pubkey,{signer:a.signer&&a.pubkey!==signed?.address,writable:a.writable}),(a.signer?1:0)|(a.writable?2:0)]);
            const call=Buffer.concat([Buffer.from([api.account(ix.program),metas.length,...metas.flat()]),u16(ix.data.length),ix.data]);
            ops.push(seed===undefined?Buffer.concat([Buffer.from([1]),call]):Buffer.concat([Buffer.from([2,seed.length]),Buffer.from(seed),call]));return api;
        },
        create(payer,created,space,seed){ops.push(Buffer.concat([Buffer.from([3,api.account(payer,{signer:true,writable:true}),
            api.account(created,{writable:true})]),u16(space),Buffer.from([seed.length]),Buffer.from(seed)]));return api;},
        write(target,offset,bytes){ops.push(Buffer.concat([Buffer.from([4,api.account(target,{writable:true})]),u16(offset),u16(bytes.length),bytes]));return api;},
        move(from,to,amount){ops.push(Buffer.concat([Buffer.from([5,api.account(from,{writable:true}),api.account(to,{writable:true})]),u64(amount)]));return api;},
        returnData(){ops.push(Buffer.from([6]));return api;},
        fail(code){ops.push(Buffer.concat([Buffer.from([7]),u32(code)]));return api;},
        log(target){ops.push(Buffer.from([8,api.account(target)]));return api;},
        clock(){ops.push(Buffer.from([9]));return api;},
        build(){return {program,accounts:accounts.map(a=>({...a})),data:Buffer.concat(ops)};},
    };
    return api;
}
/** System transfer and SPL Token transfer instructions. */
export const systemTransfer=(from,to,lamports)=>({program:"11111111111111111111111111111111",
    accounts:[{pubkey:from,signer:true,writable:true},{pubkey:to,signer:false,writable:true}],data:Buffer.concat([u32(2),u64(lamports)])});
export const tokenTransfer=(source,destination,owner,amount)=>({program:"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    accounts:[{pubkey:source,signer:false,writable:true},{pubkey:destination,signer:false,writable:true},{pubkey:owner,signer:true,writable:false}],
    data:Buffer.concat([Buffer.from([3]),u64(amount)])});
/** System CreateAccount (`owner`: 32 bytes); `created` signs (signatures are not verified on the fork or in the exact check). */
export const createAccount=(payer,created,lamports,space,owner)=>({program:"11111111111111111111111111111111",
    accounts:[{pubkey:payer,signer:true,writable:true},{pubkey:created,signer:true,writable:true}],
    data:Buffer.concat([u32(0),u64(lamports),u64(space),owner])});
const UPGRADEABLE="BPFLoaderUpgradeab1e11111111111111111111111",WRAPPED_SOL="So11111111111111111111111111111111111111112";
// Long-lived upgradeable programs, for a slot whose evidence holds none the target leaves uncalled.
const KNOWN_HOSTS=["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA","cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG","CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C"];
/**
 * Accounts for a battle around the target (`job`: its verified job's {prepared, result}), all historical and provable:
 * a host program the target does not call; a wrapped-SOL token account and two wallets the target never touches and
 * nothing changed before it; and an address the block proves empty at the target. The slot's evidence comes first;
 * otherwise the block: accounts first used after the target with a balance (so they existed, untouched before it),
 * wrapped-SOL accounts from its token balances, and an account the target itself starts empty.
 */
export function pickAccounts(job,calledPrograms,base58Encode){
    const target=new Set(job.result.original.accountTransitions.map(t=>t.pubkey)),changed=new Set(),bytes=a=>Buffer.from(a.dataBase64,"base64");
    for(const tx of job.result.prefix)for(const t of tx.accountTransitions)if(JSON.stringify(t.before)!==JSON.stringify(t.after))changed.add(t.pubkey);
    for(const fee of job.prepared.fixture.target.omittedFeeEffects)changed.add(fee.feePayer);
    const quiet=job.prepared.fixture.accounts.filter(a=>a.presence==="present"&&!target.has(a.pubkey)&&!changed.has(a.pubkey));
    const txs=JSON.parse(Buffer.from(job.prepared.rawBlockBase64,"base64")).result.transactions;
    const at=txs.findIndex(t=>t.transaction.signatures[0]===job.result.targetSignature),first=new Map(),last=new Map(),payers=[],wrappedLater=[];
    const keysOf=t=>[...t.transaction.message.accountKeys.map(k=>typeof k==="string"?k:k.pubkey),...(t.meta.loadedAddresses?.writable??[]),...(t.meta.loadedAddresses?.readonly??[])];
    txs.forEach((t,i)=>{
        const keys=keysOf(t);
        keys.forEach((k,j)=>{if(!first.has(k))first.set(k,{i,pre:t.meta.preBalances[j]});if(i<at)last.set(k,t.meta.postBalances[j]);});
        if(i<=at)return;
        if(first.get(keys[0]).i===i&&t.meta.preBalances[0]>0)payers.push(keys[0]);
        for(const b of t.meta.preTokenBalances??[])if(b.mint===WRAPPED_SOL&&b.programId==="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"&&
            first.get(keys[b.accountIndex]).i===i)wrappedLater.push(keys[b.accountIndex]);
    });
    const untouched=k=>!target.has(k)&&!changed.has(k);
    // Never a program Surfpool bundles (Token-2022): replacing it would replace Surfpool's own copy.
    const host=quiet.find(a=>a.executable&&a.owner===UPGRADEABLE&&!calledPrograms.has(a.pubkey)&&a.pubkey!=="TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")?.pubkey
        ??KNOWN_HOSTS.find(h=>!calledPrograms.has(h));
    const wrapped=quiet.filter(a=>a.owner==="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"&&bytes(a).length===165&&base58Encode(bytes(a).subarray(0,32))===WRAPPED_SOL)
        .sort((a,b)=>Number(BigInt(a.lamports)-BigInt(b.lamports))).map(a=>a.pubkey);  // smallest first: a user's, not a pool vault
    const tokenAccount=[...wrapped,...wrappedLater.filter(untouched)][0];
    const wallets=[...quiet.filter(a=>a.owner==="11111111111111111111111111111111"&&a.dataBase64===""&&BigInt(a.lamports)>10_000_000n).map(a=>a.pubkey),
        ...payers.filter(untouched)].filter((k,i,all)=>all.indexOf(k)===i);
    const startsEmpty=keysOf(txs[at]).filter((k,j)=>txs[at].meta.preBalances[j]===0&&!k.startsWith("Sysvar"));
    const empty=[...first].find(([k,v])=>v.i<at&&v.pre===0&&last.get(k)===0&&untouched(k)&&!k.startsWith("Sysvar"))?.[0]??startsEmpty[0];
    if(host===undefined||tokenAccount===undefined||wallets.length<2||empty===undefined)throw new Error("this block lacks a battle account");
    return {host,tokenAccount,wallet:wallets[0],wallet2:wallets[1],empty};
}
