import {ReplayError, type HistoricalInput, type Json, type JsonObject} from "./types.js";

function check(value: unknown, depth: number): asserts value is Json {
    if (depth > 128) throw new ReplayError("INVALID_JSON", "JSON nesting exceeds 128");
    if (typeof value === "number") {
        if (!Number.isSafeInteger(value)) throw new ReplayError("UNSAFE_NUMBER", "use decimal strings for wide integers; floats are not protocol numbers");
    } else if (value !== null && typeof value === "object") {
        if (Array.isArray(value)) for (const item of value) check(item, depth+1);
        else {
            if (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null)
                throw new ReplayError("INVALID_JSON", "protocol objects must be plain JSON, not ecosystem client objects");
            for (const item of Object.values(value)) check(item, depth+1);
        }
    } else if (value !== null && typeof value !== "string" && typeof value !== "boolean") {
        throw new ReplayError("INVALID_JSON", "use explicit JSON values; encode bigint as decimal strings");
    }
}
export function encodeJson(value: unknown): string { check(value,0); return JSON.stringify(value); }
export function decodeJson(text: string): Json {
    let value: unknown;
    try { value=JSON.parse(text); } catch (error) { throw new ReplayError("INVALID_JSON", "invalid JSON", {cause:String(error)}); }
    check(value,0); return value;
}
/** A convenience builder, not a historical-runtime oracle. Rust validates every request. */
export function historicalRequest(input: HistoricalInput): JsonObject {
    if (!["v2-2","v2-3","v3-0","v3-1","v4-0","v4-1","v4-2"].includes(input.family))
        throw new ReplayError("UNSUPPORTED_RUNTIME", "runtime family is not reviewed");
    const metadataPolicy=input.metadataPolicy === undefined ? "STRICT" : input.metadataPolicy;
    if (!["STRICT","ARCHIVED_COMPUTE_METER_WARNING"].includes(metadataPolicy))
        throw new ReplayError("INVALID_REQUEST", "unknown metadata policy");
    const limits={timeoutMs:180000,maxOutputBytes:268435456,maxDiagnosticBytes:1048576,...input.limits};
    const request={schema:"svm-replay-historical/v1",requestId:input.requestId,family:input.family,genesisHash:input.genesisHash,
        candidate:input.candidate,runtimeBinding:input.runtimeBinding,
        replacementTransactionBase64:input.replacementTransactionBase64 === undefined ? null : input.replacementTransactionBase64,
        requestedAccountOverrides:input.requestedAccountOverrides === undefined ? null : input.requestedAccountOverrides,
        bankInputs:input.bankInputs === undefined ? [] : input.bankInputs,metadataPolicy,limits};
    return decodeJson(encodeJson(request)) as JsonObject;
}
