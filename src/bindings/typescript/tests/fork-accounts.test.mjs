import {test} from "node:test";
import assert from "node:assert/strict";
import {base58Encode,base58Decode,findProgramAddress,programDataAddress} from "../dist/adapters/fork/accounts.js";
import {parseExact,stringifyExact} from "../dist/adapters/fork/json.js";
const TOKEN="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",TOKEN_2022="TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
// Historical selection/footprint tests now live beside the native boundary component.
test("base58 round-trips addresses including leading zero bytes",()=>{
    for(const key of [TOKEN,"11111111111111111111111111111111","So11111111111111111111111111111111111111112"]){
        assert.equal(base58Decode(key).length,32);assert.equal(base58Encode(base58Decode(key)),key);
    }
    assert.throws(()=>base58Decode("0OIl"),e=>e.code==="INVALID_REQUEST");
});
test("loader-v3 ProgramData addresses match mainnet",()=>{
    assert.equal(programDataAddress(TOKEN_2022),"DoU57AYuPFu2QU514RktNPG22QhApEjnKxnBcu4BHDTY");
    assert.equal(programDataAddress("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"),"4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT");
    assert.equal(programDataAddress(TOKEN),"3gvYRKWyXRR9xKWe1ZjPhLY5ZJRN7KDB4rFZFGoJfFk2");
    const ATA="ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",seeds=(...keys)=>keys.map(k=>base58Decode(k));
    assert.equal(findProgramAddress(seeds("4x97cNn7KoeYJTSWCCgaBPKZL6gWm9UQPX5X7mHHiXuh",TOKEN,"So11111111111111111111111111111111111111112"),ATA),"2HZFJ5rmsZojEuGHT2pZBK3r7dhDWT6HRKKxLsamF51v");
    assert.equal(findProgramAddress(seeds("DdHDxR7fhvHDna4ee8UBQ74hBxTk2dun4d96Bci2dh2q",TOKEN_2022,"GQnXYbbFTNJob7rzhFEFNgYMLghu4jLiK22xhrsRpump"),ATA),"2UU2E283fbCNFR4n9ijANqkvwsJVt94j6wAstcL5jGfb");
});
test("u64 values survive JSON both ways",()=>{
    const text=stringifyExact({rentEpoch:18446744073709551615n,lamports:5n});
    assert.equal(text,'{"rentEpoch":18446744073709551615,"lamports":5}');
    assert.deepEqual(parseExact(text),{rentEpoch:"18446744073709551615",lamports:5});
});
