import {test} from "node:test";
import assert from "node:assert/strict";
import {PublicKey,SystemProgram} from "@solana/web3.js";
import {offlineTransfer} from "./transfer.mjs";
test("real Anchor accountsStrict produces exact SystemProgram transfer bytes without RPC",async()=>{
    const payer=new PublicKey(new Uint8Array(32).fill(1)),recipient=new PublicKey(new Uint8Array(32).fill(2));
    const {transaction,rpcCalls}=await offlineTransfer({payer:payer.toBase58(),recipient:recipient.toBase58(),blockhash:new PublicKey(new Uint8Array(32).fill(3)).toBase58(),lamports:"1"});
    assert.equal(rpcCalls,0);
    assert.deepEqual(transaction.instructions[0],SystemProgram.transfer({fromPubkey:payer,toPubkey:recipient,lamports:1n}));
    assert.ok(transaction.serialize({requireAllSignatures:false,verifySignatures:false}).length>0);
});
