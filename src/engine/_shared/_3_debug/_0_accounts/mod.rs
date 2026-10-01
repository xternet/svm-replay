use super::{error, require, DebugClient, DebugState, InvocationMetadata};
use serde_json::{json, Value};
use svm_replay_protocol::{parse_json, Error};
mod _0_inspection;
use _0_inspection as inspection;

pub use inspection::{account_directory, inspect_account};

pub(super) use inspection::bound_accounts;
mod _1_inspect_directory;
use _1_inspect_directory as inspect_directory;

use inspect_directory::inspect_directory;
