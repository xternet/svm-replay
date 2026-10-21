use super::*;

/// One backend request; `branch` names the isolated session branch it applies to.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    #[serde(default)]
    pub branch: u64,
    pub op: Op,
    /// The embedder's store revision this operation belongs to, journaled with it so a
    /// restart can tell operations its store committed from newer ones.
    #[serde(default)]
    pub revision: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Op {
    /// A worker command on `branch`; branch selection itself stays with the backend.
    Command(SessionCommand),
    /// Copy `branch` into a new isolated branch and return its id.
    Fork,
    /// Discard `branch`.
    Close,
    /// Simulate on `branch`, optionally with the post-execution bytes of `accounts`.
    Simulate(Simulation),
    /// Replace `into` with `branch`, a fork of it, unless `into` changed since the fork.
    Commit { into: u64 },
    /// The number of state changes `branch` has had.
    Revision,
    /// What `branch` already holds for `pubkeys`, acquiring nothing: loaded or not.
    Peek { pubkeys: Vec<String> },
    /// Put `pubkeys` on `branch` back to their boundary images.
    Restore { pubkeys: Vec<String> },
    /// Every account `branch` has loaded, with its image; not every account at the slot.
    KnownAccounts,
    /// Finalized history before the fork's position, identical for every branch.
    Archive(ArchiveQuery),
    /// Close the session and exit after replying.
    Shutdown,
}

/// What the fork's history holds strictly before its position: the boundary block's
/// transactions before it, and every earlier block.
#[derive(Clone, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields,
    tag = "kind"
)]
pub enum ArchiveQuery {
    /// The boundary block's archived header and its transactions before the position.
    BoundaryBlock,
    /// An archived transaction before the position, or null; without `history` only the
    /// boundary block is consulted, as a status lookup without transaction history.
    Transaction {
        signature: String,
        #[serde(default = "consult_history")]
        history: bool,
    },
    /// An archived block before the boundary slot.
    Block { slot: u64 },
    /// An address's archived signatures before the position, newest first.
    Signatures {
        address: String,
        before: Option<String>,
        until: Option<String>,
        limit: u64,
    },
    /// The produced slots of `start..=end` before the boundary slot.
    Blocks { start: u64, end: u64 },
    /// The token accounts that may be `owner`'s at the position, by exactly one of `mint` or
    /// `programId`: the provider's list at the boundary's parent slot and every account the
    /// boundary prefix's token balances show the owner held. The embedder narrows this
    /// superset with the fork's current images.
    TokenAccountsByOwner {
        owner: String,
        mint: Option<String>,
        program_id: Option<String>,
    },
    /// `mint`'s nonzero token accounts ranked at the boundary's parent slot (at most `limit`),
    /// whether that ranking is complete, and the accounts the boundary prefix changed.
    TokenHolders { mint: String, limit: u64 },
    /// One page of `address`'s history before the position from the provider's own index,
    /// with the provider applying the status, token-account, slot and block-time filters.
    TransactionsForAddress {
        address: String,
        sort_order: SortOrder,
        token_accounts: TokenAccounts,
        status: Status,
        limit: u64,
        pagination_token: Option<String>,
        #[serde(default)]
        slot: Option<Value>,
        #[serde(default)]
        block_time: Option<Value>,
    },
}

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SortOrder {
    Asc,
    Desc,
}

/// Whether an address's history includes its token accounts' transactions.
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenAccounts {
    None,
    BalanceChanged,
    All,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Any,
    Succeeded,
    Failed,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Simulation {
    pub transaction_base64: String,
    pub sig_verify: bool,
    pub replace_recent_blockhash: bool,
    #[serde(default)]
    pub accounts: Vec<String>,
}

fn consult_history() -> bool {
    true
}

/// Decodes one line; a request whose id cannot be read is answered with a null id.
pub fn decode(line: &str) -> Result<Request, (Value, Error)> {
    let value: Value = serde_json::from_str(line).map_err(|e| {
        (
            Value::Null,
            Error::new("INVALID_BACKEND_REQUEST", format!("invalid JSON: {e}")),
        )
    })?;
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    serde_json::from_value(value)
        .map_err(|e| (id, Error::new("INVALID_BACKEND_REQUEST", e.to_string())))
}
