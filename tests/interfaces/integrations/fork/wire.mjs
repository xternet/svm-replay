/** Independent test-only no-instruction transaction encoder. Historical projection is native, not implemented here. */
export function footprintTransaction(encoded,extra,decodeAddress){
    const tx=Buffer.from(encoded,"base64");let at=0;
    const short=()=>{let n=0;for(let shift=0;;shift+=7){const byte=tx[at++];if(byte===undefined||shift>14)throw new Error("truncated test transaction");n|=(byte&127)<<shift;if(!(byte&128))return n;}};
    let payer,hash;
    if(tx[0]===0x81){payer=tx.subarray(42,74);hash=tx.subarray(8,40);}
    else{at+=64*short();if(tx[at]&128)at++;at+=3;const count=short();payer=tx.subarray(at,at+32);hash=tx.subarray(at+32*count,at+32*count+32);}
    const compact=n=>{const bytes=[];for(;;){const byte=n&127;n>>=7;bytes.push(byte|(n?128:0));if(!n)return Buffer.from(bytes);}};
    return Buffer.concat([Buffer.from([1]),Buffer.alloc(64),Buffer.from([1,0,extra.length]),compact(1+extra.length),payer,...extra.map(decodeAddress),hash,Buffer.from([0])]).toString("base64");
}
