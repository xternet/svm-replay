export type Json = null | boolean | number | string | Json[] | {[key: string]: Json};
export type JsonObject = {[key: string]: Json};
/** Stable machine IDs of the pinned runtimes; people see `RuntimeIdentity.name` (e.g. "Agave 2.3.9"). */
export type RuntimeFamily = "v2-2" | "v2-3" | "v3-0" | "v3-1" | "v4-0" | "v4-1" | "v4-2";
/** A pinned runtime as the engine names it from its executor provenance (receipts, `capabilities`). */
export interface RuntimeIdentity {
    family: RuntimeFamily; name: string; agave: string; litesvm: string; reviewedSlots: [number, number][];
}
export type Outcome = "COMPLETED" | "NEEDS_INPUT" | "UNSUPPORTED" | "MISMATCH" | "CANCELLED" | "TIMEOUT" | "ERROR";
export type MetadataPolicy = "STRICT" | "ARCHIVED_COMPUTE_METER_WARNING";
export interface Limits {timeoutMs: number; maxOutputBytes: number; maxDiagnosticBytes: number}
export interface HistoricalInput {
    requestId: string; family: RuntimeFamily; genesisHash: string;
    candidate: JsonObject; runtimeBinding: JsonObject;
    replacementTransactionBase64?: string; requestedAccountOverrides?: Json[];
    /** Transactions run after or instead of the target, on state overrides (excludes the two above). */
    whatIf?: WhatIf;
    bankInputs?: string[]; metadataPolicy?: MetadataPolicy; limits?: Partial<Limits>;
}
/**
 * An eth_call-style state override on any account, existing or new. With `owner` it replaces the
 * whole account; without it, the given fields patch the existing one. `lamports: "0"` deletes it.
 * `program: "@path/to/program.so"` (CLI and SDK only) puts that program there.
 */
export interface StateOverride {
    pubkey: string; lamports?: string; owner?: string; executable?: boolean; rentEpoch?: string;
    dataBase64?: string; program?: string;
}
/** One `{transactionBase64}` or several, run in order. */
export type BundleInput = JsonObject | JsonObject[] | {path: string};
export interface WhatIf {
    mode: "after" | "replace"; transactionsBase64?: string[]; stateOverrides?: Json[];
}
export interface Artifact {file: string; sha256: string}
export interface ArtifactHandle extends Artifact {path: string; bytes: number}
export type IncompleteCategory = "HISTORICAL_ACCOUNT_MISSING" | "SLOT_HASHES_MISSING"
    | "SYSVAR_MISSING" | "BANK_CONTEXT_MISSING" | "HISTORICAL_DATA_UNAVAILABLE"
    | "RUNTIME_UNSUPPORTED" | "CAPABILITY_UNSUPPORTED" | "DEPENDENCY_UNSUPPORTED"
    | "RESOURCE_LIMIT" | "PROVIDER_ERROR" | "VERIFICATION_MISMATCH" | "TIMEOUT"
    | "CANCELLED" | "UNCLASSIFIED";
export interface IncompleteEvidence {
    schema: "svm-replay-incomplete-evidence/v1"; category: IncompleteCategory;
    complete: false; estimated: false; requestedReplayVerified: false;
    originalControlVerified: boolean; input: JsonObject; available: JsonObject; meaning: string;
}
export interface Receipt {
    schema: string; outcome: Outcome; requestId?: string; requestIdentity?: string;
    implementationSha256?: string; processOwnerSha256?: string; currentStateFallback?: boolean;
    receiptPath?: string; output?: Artifact; controlVerification?: {status: string}; verification?: {status: string};
    error?: {code: string; message: string; details?: unknown};
    incomplete?: IncompleteEvidence;
    /** The runtime that executed the job by official name (0.2.0 and later); null for an unpinned executor. */
    runtime?: RuntimeIdentity | null;
    [key: string]: unknown;
}
export interface Source {path: string; sha256: string}
export interface ReplayOptions {
    bundlePath: string; bundleSha256: string; dataDir?: string;
    source?: Source; alchemyConfig?: string; cache?: "off" | "prepared" | "all";
}
export interface SimulateOptions {
    request: JsonObject | {path: string}; source?: Source; alchemyConfig?: string;
    genesisHash?: string; trace?: JsonObject | {path: string}; cache?: "off" | "prepared" | "all";
    signal?: AbortSignal; timeoutMs?: number; onProgress?: (event:ProgressEvent)=>void;
}
export interface Simulation {receipt: Receipt; result?: ArtifactHandle; stderr: string}
export interface SignatureOptions {
    /** Run these instead of the target; `append` runs them right after it. */
    tx:string; replace?:BundleInput; append?:BundleInput; overrides?:Json[]|{path:string};
    collect?:"all"|JsonObject|{path:string}; fields?:Array<"status"|"logs"|"computeUnits"|"accountTransitions"|"trace">;
    noCache?:boolean; alchemyKeyFile?:string; alchemyConfig?:string; signal?:AbortSignal; timeoutMs?:number; onProgress?: (event:ProgressEvent)=>void;
    maxRequests?:number; maxDownloadBytes?:number; maxOutputBytes?:number;
    bankInputs?:string[]; source?:Source; runtimeRegistry?:Source;
}
export interface SignatureSimulation extends Simulation {response?:JsonObject}
export class ReplayError extends Error {
    constructor(readonly code: string, message: string, readonly details: Record<string,unknown> = {}) {
        super(message); this.name = "ReplayError";
    }
}

export type ProgressPhase = "locating_transaction" | "resolving_dependencies" | "fetching_historical_accounts"
    | "replaying_earlier_transactions" | "verifying_original" | "saving_inputs" | "executing_requested";
export interface ProgressTotal {value:number;status:"provisional"|"final"}
export interface ProgressEvent {
    schema:"svm-replay-progress/v1"; origin:"native"|"sdk"; sequence:number;
    kind:"phase"|"advance"|"heartbeat"|"terminal"; phase?:ProgressPhase;
    elapsedMs:number; phaseDurationMs?:number; sinceAdvanceMs:number; attempt:number;
    fetchedAccounts?:number; cacheHits?:number; downloadedBytes?:number; downloadedBytesKnown?:boolean;
    processedEarlierTransactions?:number; earlierTransactionsTotal?:ProgressTotal; outcome?:Outcome;
}
