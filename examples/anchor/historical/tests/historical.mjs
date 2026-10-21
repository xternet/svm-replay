// Runs under `anchor test` against Surfpool forked just before a historical transaction. No dependencies.
// HISTORICAL_EXPECT names a JSON file with the verified expectations (see ../README.md).
import {test} from "node:test";
import assert from "node:assert/strict";
import {createHash,createPrivateKey,sign} from "node:crypto";
import {readFileSync} from "node:fs";

const url=process.env.ANCHOR_PROVIDER_URL,expect=JSON.parse(readFileSync(process.env.HISTORICAL_EXPECT,"utf8"));
const ALPHABET="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const decode=text=>{let n=0n;for(const c of text)n=n*58n+BigInt(ALPHABET.indexOf(c));
    const hex=n.toString(16).padStart(64,"0");return Buffer.from(hex,"hex");};
async function rpc(method,params=[]){
    const response=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})});
    const body=await response.json();if(body.error)throw new Error(`${method}: ${body.error.message}`);return body.result;
}
/** Unsigned legacy transaction (Surfpool simulates with sigVerify off): payer, one instruction, read-only accounts. */
function transaction(payer,program,accounts,data,blockhash){
    const keys=[payer,...accounts,program].map(decode);
    const instruction=Buffer.concat([Buffer.from([keys.length-1,accounts.length]),Buffer.from(accounts.map((_,i)=>i+1)),Buffer.from([data.length]),data]);
    const message=Buffer.concat([Buffer.from([1,0,accounts.length+1,keys.length]),...keys,decode(blockhash),Buffer.from([1]),instruction]);
    return Buffer.concat([Buffer.from([1]),Buffer.alloc(64),message]).toString("base64");
}
const simulate=tx=>rpc("simulateTransaction",[tx,{encoding:"base64",sigVerify:false,replaceRecentBlockhash:true}]).then(r=>r.value);
/** Signs with the provider wallet (ANCHOR_WALLET: 32-byte secret seed, then the public key). */
function signed(tx){
    const seed=Buffer.from(JSON.parse(readFileSync(process.env.ANCHOR_WALLET,"utf8")).slice(0,32));
    const key=createPrivateKey({key:Buffer.concat([Buffer.from("302e020100300506032b657004220420","hex"),seed]),format:"der",type:"pkcs8"});
    const bytes=Buffer.from(tx,"base64");sign(null,bytes.subarray(65),key).copy(bytes,1);return bytes.toString("base64");
}
async function landed(signature){
    for(let i=0;i<40;i++){
        const found=await rpc("getTransaction",[signature,{encoding:"json",commitment:"confirmed",maxSupportedTransactionVersion:0}]);
        if(found!==null)return found;await new Promise(r=>setTimeout(r,250));
    }
    throw new Error(`${signature} did not land`);
}
const observe=createHash("sha256").update("global:observe").digest().subarray(0,8);

test("historical account is the verified image, not today's",async()=>{
    const {value}=await rpc("getAccountInfo",[expect.account.pubkey,{encoding:"base64"}]);
    assert.equal(createHash("sha256").update(Buffer.from(value.data[0],"base64")).digest("hex"),expect.account.dataSha256);
    assert.equal(String(value.lamports),expect.account.lamports);
});
test("a program deployed by anchor sees the historical clock and state",async()=>{
    const {blockhash}=(await rpc("getLatestBlockhash")).value;
    const result=await simulate(transaction(expect.wallet,expect.programId,[expect.account.pubkey],observe,blockhash));
    assert.equal(result.err,null);
    if(expect.matchDatasource)assert.ok(result.logs.includes(`Program log: unix_timestamp=${expect.clock.unixTimestamp} slot=${expect.clock.slot}`),result.logs.join("\n"));
    assert.ok(result.logs.includes(`Program log: lamports=${expect.account.lamports}`));
});
test("the historical transaction re-runs with the verified outcome",{skip:!expect.matchDatasource&&"Stock Surfpool keeps today's Clock and bundled programs"},async()=>{
    const result=await rpc("simulateTransaction",[expect.original.transactionBase64,{encoding:"base64",sigVerify:false,replaceRecentBlockhash:false}]);
    assert.equal(result.value.err===null,expect.original.status==="ok");
    if(expect.original.status!=="ok")assert.equal(JSON.stringify(result.value.err),expect.original.normalizedError);
    assert.equal(result.value.unitsConsumed,expect.original.computeUnits);
    assert.deepEqual(result.value.logs,expect.original.logs);
});
test("transactions sent to the fork run on the historical state, one after another",async()=>{
    // Stock Surfpool rebuilds its clock from wall time at every block; with --match-datasource it continues from history.
    const historical=BigInt(expect.clock.unixTimestamp),matched=readFileSync("Anchor.toml","utf8").includes('"--match-datasource"');
    for(let i=0;i<2;i++){
        const {blockhash}=(await rpc("getLatestBlockhash")).value;
        const signature=await rpc("sendTransaction",[signed(transaction(expect.wallet,expect.programId,[expect.account.pubkey],observe,blockhash)),
            {encoding:"base64",skipPreflight:true}]);
        const {meta}=await landed(signature);
        assert.equal(meta.err,null);assert.ok(meta.logMessages.includes(`Program log: lamports=${expect.account.lamports}`));
        const time=BigInt(/unix_timestamp=(-?\d+)/.exec(meta.logMessages.join("\n"))[1]);
        assert.ok(time>=historical,`transaction ${i+1} ran before the historical moment: ${time}`);
        if(matched)assert.ok(time<=historical+120n,`transaction ${i+1} clock ${time} is not historical (${historical})`);
    }
});
