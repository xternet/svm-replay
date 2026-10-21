use super::super::{
    bank::{context, imports, migration, restart, snapshots, stake, sysvars},
    config::Config,
    history::{self, History, Roles, CLOCK},
    runtime::ExecutionBudget,
    sources::CompositeSource,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use svm_replay_protocol::{Digest, Error, Limits};
mod _0_request;
use _0_request::position;
pub use _0_request::InitializationRequest;
mod _1_images;
mod _2_fixture;
pub use _2_fixture::prepare;
mod _3_validate;
pub(super) use _3_validate::validate;
mod _4_persist;
pub use _4_persist::initialize;
mod _5_execute;
pub use _5_execute::execute;

fn invalid(message: impl Into<String>) -> Error {
    Error::new("INVALID_INITIALIZED_SLOT", message)
}
