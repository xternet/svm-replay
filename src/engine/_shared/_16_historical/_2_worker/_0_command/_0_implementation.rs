use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire commands only; no duplicate Bank, provider or runtime implementation.
#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "method",
    content = "params",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum SessionCommand {
    State,
    Accounts {
        pubkeys: Vec<String>,
    },
    ConfigureRpc {
        context: Value,
    },
    SetBalance {
        pubkey: String,
        lamports: String,
    },
    OverrideAccounts {
        entries: Vec<svm_replay_protocol::StateOverride>,
    },
    Hydrate {
        fixture: Value,
        original: Value,
        #[serde(rename = "stateSha256")]
        state_sha256: String,
        #[serde(rename = "receiptSha256")]
        receipt_sha256: String,
        after: bool,
    },
    RpcSimulate {
        #[serde(rename = "transactionBase64")]
        transaction_base64: String,
        #[serde(rename = "sigVerify")]
        sig_verify: bool,
        #[serde(rename = "replaceRecentBlockhash")]
        replace_recent_blockhash: bool,
    },
    RpcSend {
        #[serde(rename = "transactionBase64")]
        transaction_base64: String,
        #[serde(rename = "skipPreflight")]
        skip_preflight: bool,
    },
    LatestBlockhash,
    SignatureStatuses {
        signatures: Vec<String>,
    },
    Rent {
        #[serde(rename = "dataLen")]
        data_len: usize,
    },
    FeeForMessage {
        #[serde(rename = "messageBase64")]
        message_base64: String,
    },
    /// Whether a blockhash would be accepted for processing on the active fork now.
    BlockhashValid {
        blockhash: String,
    },
    /// Pinned parent headers proving the boundary queue's ages, newest first.
    ProveBlockhashQueue {
        headers: Value,
    },
    Simulate {
        #[serde(rename = "transactionBase64")]
        transaction_base64: String,
        #[serde(rename = "sigVerify")]
        sig_verify: bool,
    },
    Send {
        #[serde(rename = "transactionBase64")]
        transaction_base64: String,
        #[serde(rename = "sigVerify")]
        sig_verify: bool,
    },
    /// Explicit local slots inside the boundary's epoch; time moves only when given.
    Advance {
        slots: u64,
        #[serde(rename = "unixTimestamp")]
        unix_timestamp: Option<i64>,
        /// The explicit opt-in to a synthetic test warp across epochs: the Clock enters the
        /// later epoch while runtime, features, stakes and rewards stay at the boundary.
        /// Omitted when false, so ordinary advances keep their wire form.
        #[serde(
            rename = "syntheticEpoch",
            default,
            skip_serializing_if = "std::ops::Not::not"
        )]
        synthetic_epoch: bool,
    },
    Snapshot,
    Select {
        #[serde(rename = "forkId")]
        fork_id: u64,
    },
    Reset,
    CloseFork {
        #[serde(rename = "forkId")]
        fork_id: u64,
    },
    Close,
}
