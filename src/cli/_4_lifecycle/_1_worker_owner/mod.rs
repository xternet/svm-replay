//! CLI-only process-tree owner. Never call this from an embedded library host.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use std::ffi::OsString;

#[cfg(target_os = "macos")]
mod _0_macos;
#[cfg(target_os = "macos")]
use _0_macos as macos;
#[cfg(target_os = "macos")]
pub use macos::run;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn run(_: &[OsString]) -> Result<i32, String> {
    Err("worker owner requires Linux subreaper support".into())
}

#[cfg(target_os = "linux")]
mod _1_linux;
#[cfg(target_os = "linux")]
use _1_linux as linux;
#[cfg(target_os = "linux")]
pub use linux::run;
