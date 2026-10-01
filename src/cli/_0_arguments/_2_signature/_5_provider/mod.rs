use super::controls::Controls;
use std::path::Path;
use std::time::Duration;
use svm_replay_engine::{
    shared::{
        history::source_error,
        runtime::CancellationToken,
        sources::{
            alchemy::{AlchemyConfig, AlchemySource, SourceLimits},
            HistoricalSource,
        },
    },
    CacheMode,
};
use svm_replay_protocol::Error;
mod _0_limits;

pub(super) fn open(
    controls: &Controls,
    data: &Path,
    cache: CacheMode,
    remaining: Duration,
    cancel: CancellationToken,
) -> Result<AlchemySource, Error> {
    let mut provider = match &controls.alchemy_config {
        Some(path) => crate::sources::alchemy_source(path, data, remaining.as_millis() as u64)?,
        None => AlchemySource::from_environment(
            AlchemyConfig {
                id: "alchemy-mainnet-archive".into(),
                version: "v1".into(),
                expected_genesis_hash: "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d".into(),
                first_slot: 0,
                last_slot: 9_007_199_254_740_991,
            },
            _0_limits::source_limits(controls, remaining),
        )
        .map_err(source_error)?,
    }
    .with_cancellation(cancel);
    if provider.identity()["genesisHash"] != "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d" {
        return Err(Error::new(
            "SOURCE_CONTEXT_MISMATCH",
            "signature runtime registry requires Solana mainnet",
        ));
    }
    if cache != CacheMode::Off {
        svm_replay_engine::_5_finalize::ensure_private_directory(data)?;
        let path = svm_replay_engine::shared::layout::cache_dir(data)?;
        let store = svm_replay_store::Store::open(&path)
            .map_err(|e| Error::new(format!("STORE_{}", e.code), e.message))?;
        provider = provider.with_discovery_cache(store);
    }
    Ok(provider)
}
