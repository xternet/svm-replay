//! Resolve recorded instruction references; missing CPI history is not an empty trace.
use super::history::RECENT_BLOCKHASHES;
use serde_json::{json, Value};
use svm_replay_protocol::Error;
mod _0_analysis;
use _0_analysis as analysis;

pub use analysis::{
    classify, is_unexecuted_load_failure, requirements, resolve, Instruction, SLOT_HASHES,
    SLOT_HISTORY,
};

use analysis::bad;
mod _1_analyze_instructions_sysvar;
use _1_analyze_instructions_sysvar as analyze_instructions_sysvar;

pub use analyze_instructions_sysvar::analyze_instructions_sysvar;
