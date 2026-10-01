//! Finite historical loader input inspection, not authority or execution validation.
use super::{safe_integer, Result, SemanticTransaction};
use crate::shared::history::{account_data, classify_account, Roles};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use svm_replay_protocol::Error;
mod _0_instructions;
use _0_instructions as instructions;

pub use instructions::{HistoricalLoaderContext, IncludedTransaction, ALT, LOADER, LOADER_V4};

use instructions::{check, invalid, resolve, Instruction};

pub(super) use instructions::unsupported;
mod _1_parent;
use _1_parent as parent;

use parent::{
    authority, buffer, deploy, earlier, earlier_deploy, exact, initialized, mapping, parent, tag,
    uint,
};
mod _2_inspection;
use _2_inspection as inspection;

pub use inspection::assert_inspected_loader_lifecycle;
mod _3_accounts;
use _3_accounts as accounts;

pub use accounts::{assert_program_lifecycle_supported, build_inspected_loader_parent_accounts};
