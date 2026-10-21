/** Helpers for the user-flow matrix: keys, signed transfers, account fields and branch names. No dependencies. */
import assert from "node:assert/strict";
import {generateKeyPairSync,sign} from "node:crypto";

/** A fresh ed25519 keypair as a developer creates one: {pubkey (base58), sign(message)}. */
export function keypair(base58Encode){
    const {publicKey,privateKey}=generateKeyPairSync("ed25519");
    const raw=publicKey.export({format:"der",type:"spki"}).subarray(-32);
    return {pubkey:base58Encode(raw),sign:message=>sign(null,message,privateKey)};
}
const compact=n=>Buffer.from([n]);  // every count here is below 128
/** A signed legacy System transfer of `lamports` from `from` (keypair) to `to`. */
export function transfer(from,to,lamports,blockhash,base58Decode){
    const data=Buffer.alloc(12);data.writeUInt32LE(2,0);data.writeBigUInt64LE(BigInt(lamports),4);
    const keys=[from.pubkey,to,"11111111111111111111111111111111"].map(base58Decode);
    const message=Buffer.concat([Buffer.from([1,0,1]),compact(3),...keys,base58Decode(blockhash),compact(1),
        Buffer.from([2]),compact(2),Buffer.from([0,1]),compact(data.length),data]);
    return Buffer.concat([compact(1),from.sign(message),message]).toString("base64");
}
/** SPL mint supply (u64 at byte 36) as a decimal string. */
export const mintSupply=data=>Buffer.from(data,"base64").readBigUInt64LE(36).toString();
/** Agave's Rent::minimum_balance for `size` bytes under a Rent sysvar image. */
export function rentExemption(rentData,size){
    const rent=Buffer.from(rentData,"base64");
    return String(Math.floor(Number(BigInt(128+size)*rent.readBigUInt64LE(0))*rent.readDoubleLE(8)));
}
/** Loader-v3 ProgramData upgrade authority (Option<Pubkey> after the 4-byte tag and 8-byte slot), or null. */
export function upgradeAuthority(data,base58Encode){
    const bytes=Buffer.from(data,"base64");
    return bytes[12]===1?base58Encode(bytes.subarray(13,45)):null;
}
/** Clock unix_timestamp (i64 at byte 32). */
export const clockTime=data=>Buffer.from(data,"base64").readBigInt64LE(32);
/** Clock slot (u64 at byte 0). */
export const clockSlot=data=>Buffer.from(data,"base64").readBigUInt64LE(0);
/**
 * Runs one flow and names its branch: "ok" (the historical answer), "explicit" (Surfpool returned an error to the
 * developer) or "silent" (an answer that is not the historical one). Any other exception is a defect in the flow.
 */
export async function attempt(run){
    try {return (await run())??{branch:"ok"};}
    catch(error){
        if(error?.code==="SURFPOOL_RPC")return {branch:"explicit",detail:error.message.slice(0,300)};
        if(error instanceof assert.AssertionError)return {branch:"silent",detail:error.message.slice(0,600)};
        throw error;
    }
}
