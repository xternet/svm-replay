use super::*;

pub type Result<T> = std::result::Result<T, Error>;

pub const VOTE_PROGRAM: &str = "Vote111111111111111111111111111111111111111";

pub(in super::super) const EXCLUDED: &[&str] = &[
    VOTE_PROGRAM,
    "SysvarRecentB1ockHashes11111111111111111111",
    "SysvarS1otHashes111111111111111111111111111",
    "SysvarS1otHistory11111111111111111111111111",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTransaction {
    pub index: u64,
    pub signature: String,
    pub fee_payer: String,
    pub declared_accounts: Vec<String>,
    pub instruction_accounts: Vec<String>,
    pub writable_accounts: Vec<String>,
    pub application_writable_accounts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible_persistent_writes: Option<Vec<String>>,
    pub address_table_accounts: Vec<String>,
    pub program_ids: Vec<String>,
    pub program_data_accounts: Vec<String>,
    pub semantic_dependency_accounts: Vec<String>,
    pub excluded_dependency_accounts: Vec<String>,
    pub fee_only_payer: bool,
    pub succeeded: bool,
    pub is_vote: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WritableIntersection {
    pub index: u64,
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticClosureResult {
    pub seed_accounts: Vec<String>,
    pub selected_indices: Vec<u64>,
    pub application_dependency_accounts: Vec<String>,
    pub dependency_accounts: Vec<String>,
    pub program_data_accounts: Vec<String>,
    pub excluded_dependency_accounts: Vec<String>,
    pub fee_only_payers_ignored: Vec<String>,
    pub included_vote_indices: Vec<u64>,
    pub writable_intersections: Vec<WritableIntersection>,
    pub iterations: usize,
}

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("DEPENDENCY_INPUT", message)
}

pub(in super::super) fn safe_integer(value: u64, label: &str) -> Result<u64> {
    if value > 9_007_199_254_740_991 {
        return Err(invalid(format!(
            "{label} is not a safe non-negative integer"
        )));
    }
    Ok(value)
}

pub(in super::super) fn pubkey(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        return Err(invalid(format!("{label} is empty")));
    }
    Ok(())
}

pub(in super::super) fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

pub(in super::super) fn unique_sorted(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = unique(values);
    // Match JavaScript's default UTF-16 string sort for the metadata's string keys.
    values.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    values
}
