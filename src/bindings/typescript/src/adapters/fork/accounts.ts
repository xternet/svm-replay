//! Account helpers: base58, program-derived addresses and historical companion accounts.
import {createHash} from "node:crypto";
import {ReplayError} from "../../types.js";

export const UPGRADEABLE="BPFLoaderUpgradeab1e11111111111111111111111";
const ALPHABET="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

export function base58Encode(bytes:Uint8Array):string{
    let n=BigInt(`0x${Buffer.from(bytes).toString("hex")||"0"}`),out="";
    while(n>0n){out=ALPHABET[Number(n%58n)]+out;n/=58n;}
    for(const byte of bytes){if(byte!==0)break;out=`1${out}`;}
    return out;
}
export function base58Decode(text:string):Buffer{
    let n=0n;
    for(const char of text){const digit=ALPHABET.indexOf(char);if(digit<0)throw new ReplayError("INVALID_REQUEST",`invalid base58 address ${text}`);n=n*58n+BigInt(digit);}
    const hex=n===0n?"":n.toString(16);
    const body=Buffer.from(hex.length%2===1?`0${hex}`:hex,"hex"),zeros=text.length-text.replace(/^1+/,"").length;
    return Buffer.concat([Buffer.alloc(zeros),body]);
}
const P=2n**255n-19n;
function power(base:bigint,exponent:bigint):bigint{
    let result=1n;base%=P;
    for(;exponent>0n;exponent>>=1n){if(exponent&1n)result=result*base%P;base=base*base%P;}
    return result;
}
/** ed25519 decompression test (curve25519-dalek semantics): y with u/v a square (or zero) lies on the curve. */
function onCurve(bytes:Buffer):boolean{
    const y=BigInt(`0x${Buffer.from(bytes).reverse().toString("hex")}`)&((1n<<255n)-1n);
    const d=(P-121665n)*power(121666n,P-2n)%P,yy=y*y%P,u=(yy-1n+P)%P,v=(d*yy+1n)%P;
    const w=u*power(v,P-2n)%P;
    return w===0n||power(w,(P-1n)/2n)===1n;
}
/** The first off-curve program-derived address of `seeds` under `programId` (Solana's find_program_address). */
export function findProgramAddress(seeds:readonly Uint8Array[],programId:string):string{
    const program=base58Decode(programId);
    for(let bump=255;bump>=0;bump--){
        const candidate=createHash("sha256").update(Buffer.concat([...seeds,Buffer.from([bump]),program,Buffer.from("ProgramDerivedAddress")])).digest();
        if(!onCurve(candidate))return base58Encode(candidate);
    }
    throw new ReplayError("BOUNDARY_FORMAT",`no program-derived address under ${programId}`);
}
/** Loader-v3 ProgramData address of a program id: the first off-curve PDA of [program id]. */
export function programDataAddress(programId:string):string{
    return findProgramAddress([base58Decode(programId)],UPGRADEABLE);
}
