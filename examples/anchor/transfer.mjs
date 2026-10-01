import anchor from "@anchor-lang/core";
import {Connection,PublicKey} from "@solana/web3.js";
const {Program,BN}=anchor;

/** System transfer expressed through an explicit IDL; no keypair or wallet is needed. */
export async function offlineTransfer({payer,recipient,blockhash,lamports}){
    let rpcCalls=0;
    const connection=new Connection("http://127.0.0.1:9",{fetch:async()=>{rpcCalls++;throw new Error("unexpected provider call in strict encoding");}});
    const idl={address:"11111111111111111111111111111111",metadata:{name:"system",version:"0.1.0",spec:"0.1.0"},
        instructions:[{name:"transfer",discriminator:[2,0,0,0],accounts:[{name:"from",writable:true,signer:true},{name:"to",writable:true}],args:[{name:"lamports",type:"u64"}]}]};
    const program=new Program(idl,{connection,publicKey:new PublicKey(payer)});
    const transaction=await program.methods.transfer(new BN(lamports)).accountsStrict({from:new PublicKey(payer),to:new PublicKey(recipient)}).transaction();
    transaction.feePayer=new PublicKey(payer);transaction.recentBlockhash=blockhash;
    return {transaction,rpcCalls};
}
