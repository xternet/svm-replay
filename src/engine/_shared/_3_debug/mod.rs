//! Bounded, read-only live interpreter debugging. VM stops are not transaction completion.
mod _0_accounts;
use _0_accounts as accounts;
mod _1_client;
use _1_client as client;
mod _2_metadata;
use _2_metadata as metadata;
mod _3_navigation;
use _3_navigation as navigation;
mod _4_session;
use _4_session as session;
mod _5_store;
use _5_store as store;
mod _6_symbols;
use _6_symbols as symbols;
pub use accounts::{account_directory, inspect_account};
pub use client::{DebugClient, DebugState, Stop};
pub use metadata::{validate_finalized_inventory, FinalizedInventory, FinalizedInventoryPolicy};
pub use metadata::{validate_finalized_invocations, InvocationIdentity, InvocationMetadata};
pub use navigation::{SourceNavigation, SourceNavigationKind};
pub use session::{
    channel, run_session, DebugAction, DebugCommand, DebugController, DebugDriver, LiveExecution,
    PauseToken,
};
pub use store::{
    branch_session, open_session, save_session, FileReference, SaveOptions, SavedSession,
    SessionLineage, SessionManifest, SessionPin, SessionSymbols,
};
pub use store::{open_run, save_run, RunArtifacts, RunManifest, SavedRun};
pub use symbols::{ExactSymbols, RuntimeFrame, RuntimeFrames, SourceLocation};

use svm_replay_protocol::Error;
fn error(code: &str, message: impl Into<String>) -> Error {
    Error::new(code, message)
}
fn require(condition: bool, code: &str, message: &str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(error(code, message))
    }
}
