//! Exact-ELF source observations. Embedded paths are display data, never IO instructions.
use super::{error, require, DebugClient, DebugState, InvocationMetadata};
use crate::shared::runtime::{protocol_error, read_bounded_file};
use gimli::{AttributeValue, EndianArcSlice, LittleEndian, Reader, Section};
use object::{Object, ObjectSection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use svm_replay_protocol::{parse_json, Digest, Error};
mod _0_types;
use _0_types as types;

use types::{address, dwarf_error, text, DwarfReader, Function, Line, Variable};

pub use types::{ExactSymbols, RuntimeFrame, RuntimeFrames, SourceLocation};
mod _1_available;
mod _2_symbols;
mod _3_variable;
mod _4_variable_type;
