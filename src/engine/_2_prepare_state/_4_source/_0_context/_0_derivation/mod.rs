use super::*;

pub(in super::super) struct Boundary<'a> {
    pub a: &'a super::super::super::analysis::Analysis,
    pub envelope: &'a Value,
    pub raw_bytes: &'a [u8],
    pub closure: &'a dependencies::SemanticClosureResult,
    pub requirements: &'a Value,
    pub roles: &'a Roles,
    pub created: &'a requested::CreatedAltParents,
    pub programs: &'a [String],
    pub changes: &'a [overrides::RequestedAccountOverride],
    pub migrations: &'a Value,
    pub supplemental: &'a BTreeMap<String, Value>,
    pub parent: Vec<Value>,
    pub original_wire: &'a str,
    pub selected: &'a [dependencies::SemanticTransaction],
    pub dependency_transactions: &'a [dependencies::SemanticTransaction],
    pub lifecycle_indices: Vec<u64>,
    pub credit_proofs: &'a [Value],
    pub header_proofs: &'a [Value],
}
