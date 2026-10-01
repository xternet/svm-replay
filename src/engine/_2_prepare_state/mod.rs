//! Validate explicitly supplied prepared state before the first native replay slice.
pub use svm_replay_protocol::transaction;
pub mod _0_analysis;
pub use _0_analysis as analysis;
pub mod _1_boundary;
pub use _1_boundary as boundary;
pub mod _2_context;
pub use _2_context as context;
pub mod _3_discovery;
pub use _3_discovery as discovery;
pub mod _4_source;
pub use _4_source as source;

use serde_json::{json, Value};
use svm_replay_protocol::{Error, PreparedRequest};
mod _5_controls;
use _5_controls as controls;

pub use controls::{has_variant, original_control, validate_boundary};
