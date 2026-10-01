use super::*;

mod _0_implementation;

pub(super) use _0_implementation::{ASSURANCE, MAX_BUNDLE, MAX_FILE, MAX_MANIFEST, MAX_SYMBOLS};

pub use _0_implementation::{
    FileReference, SaveOptions, SavedSession, SessionLineage, SessionManifest, SessionPin,
    SessionSymbols,
};
