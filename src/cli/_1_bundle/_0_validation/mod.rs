use super::*;

mod _0_implementation;
#[cfg(test)]
pub(super) use _0_implementation::is_executable;

pub(super) use _0_implementation::{
    binary_name, error, manifest, regular, relative, set_mode, Entry, Manifest,
};

pub use _0_implementation::Installed;
