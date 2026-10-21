//! Version-independent Solana wire decoding; execution/admission stays in SVM.
use crate::Error;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
mod _0_decode;
mod _1_v1;
mod _2_encode;
use _0_decode as decode;

pub use _2_encode::encode;
pub use decode::{assert_historical, decode};
