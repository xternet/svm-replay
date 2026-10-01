//! Presentation only: never add ANSI sequences to JSON or saved artifacts.
use dialoguer::theme::Theme;
use std::{fmt, io::IsTerminal};

mod _0_implementation;
#[cfg(test)]
pub use _0_implementation::highlight;

pub use _0_implementation::{path, stderr, stdout, MenuTheme};
