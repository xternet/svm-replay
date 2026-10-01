//! Cooperative signals belong only to the standalone CLI, never the SDK host.
use svm_replay_engine::shared::runtime::CancellationToken;
use svm_replay_protocol::Error;

#[cfg(any(unix, windows))]
static SIGNAL_TOKEN: std::sync::OnceLock<CancellationToken> = std::sync::OnceLock::new();

#[cfg(unix)]
mod _0_unix;
#[cfg(unix)]
use _0_unix as unix;
#[cfg(unix)]
pub use unix::install;
#[cfg(windows)]
mod _1_windows;
#[cfg(windows)]
use _1_windows as windows;
#[cfg(windows)]
pub use windows::install;

#[cfg(not(any(unix, windows)))]
pub fn install() -> Result<CancellationToken, Error> {
    Err(Error::new(
        "UNSUPPORTED_PLATFORM",
        "CLI cancellation is currently qualified only on Linux",
    ))
}
