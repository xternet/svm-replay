import {historicalRequest} from "../request.js";
import {ReplayError,type HistoricalInput,type SignatureOptions} from "../types.js";

/** Anchor-built replacement against an automatically reconstructed historical signature. */
export function anchorSignature(tx:string,transaction:SerializedTransaction):SignatureOptions {
    const bytes=transaction.serialize({requireAllSignatures:false,verifySignatures:false});
    if(!(bytes instanceof Uint8Array)||bytes.length===0)throw new ReplayError("INVALID_REQUEST","transaction serialized to no bytes");
    return {tx,replace:{transactionBase64:Buffer.from(bytes).toString("base64")}};
}
export interface SerializedTransaction {serialize(options:{requireAllSignatures:false;verifySignatures:false}):Uint8Array}
/** Caller must already have supplied strict accounts, fee payer and historical blockhash. No wallet/provider calls. */
export function anchorRequest(input:HistoricalInput,transaction:SerializedTransaction){
    const bytes=transaction.serialize({requireAllSignatures:false,verifySignatures:false});
    if(!(bytes instanceof Uint8Array)||bytes.length===0)throw new ReplayError("INVALID_REQUEST","transaction serialized to no bytes");
    return historicalRequest({...input,replacementTransactionBase64:Buffer.from(bytes).toString("base64")});
}
