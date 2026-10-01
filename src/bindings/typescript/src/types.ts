export type Json = null | boolean | number | string | Json[] | {[key: string]: Json};
export type JsonObject = {[key: string]: Json};
export type RuntimeFamily = "v2-2" | "v2-3" | "v3-0" | "v3-1" | "v4-0" | "v4-1" | "v4-2";
export type Outcome = "COMPLETED" | "NEEDS_INPUT" | "UNSUPPORTED" | "MISMATCH" | "CANCELLED" | "TIMEOUT" | "ERROR";
export type MetadataPolicy = "STRICT" | "ARCHIVED_COMPUTE_METER_WARNING";
export interface Limits {timeoutMs: number; maxOutputBytes: number; maxDiagnosticBytes: number}
export interface HistoricalInput {
    requestId: string; family: RuntimeFamily; genesisHash: string;
    candidate: JsonObject; runtimeBinding: JsonObject;
    replacementTransactionBase64?: string; requestedAccountOverrides?: Json[];
    bankInputs?: string[]; metadataPolicy?: MetadataPolicy; limits?: Partial<Limits>;
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
    signal?: AbortSignal; timeoutMs?: number;
}
export interface Simulation {receipt: Receipt; result?: ArtifactHandle; stderr: string}
export interface SignatureOptions {
    tx:string; replace?:JsonObject|{path:string}; overrides?:Json[]|{path:string};
    collect?:"all"|JsonObject|{path:string}; fields?:Array<"status"|"logs"|"computeUnits"|"accountTransitions"|"trace">;
    noCache?:boolean; alchemyKeyFile?:string; alchemyConfig?:string; signal?:AbortSignal; timeoutMs?:number;
    maxRequests?:number; maxDownloadBytes?:number; maxOutputBytes?:number;
    bankInputs?:string[]; source?:Source; runtimeRegistry?:Source;
}
export interface SignatureSimulation extends Simulation {response?:JsonObject}
export class ReplayError extends Error {
    constructor(readonly code: string, message: string, readonly details: Record<string,unknown> = {}) {
        super(message); this.name = "ReplayError";
    }
}
