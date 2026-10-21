use super::*;
use crate::arguments::help;
use svm_replay_engine::shared::historical::{ProviderOptions, ResolvedProvider};

#[derive(Args, Default)]
#[group(id = "HistoricalProviderOptions")]
pub(crate) struct Options {
    /// Named provider/adapter, not an API key (currently alchemy).
    #[arg(long = "provider", alias = "historical-provider", value_name = "NAME", help_heading = help::PROVIDER, requires = "replay_target", conflicts_with = "alchemy_config")]
    historical_provider: Option<String>,
    /// Optional provider TOML; defaults to ./svm-replay.toml when present.
    #[arg(long = "config", alias = "historical-config", value_name = "FILE", help_heading = help::PROVIDER, requires = "replay_target", conflicts_with = "alchemy_config")]
    historical_config: Option<PathBuf>,
    /// API key or env:VARIABLE; prefer env/file to avoid shell history exposure.
    #[arg(long = "key", alias = "historical-key", value_name = "KEY|env:NAME", help_heading = help::PROVIDER, requires = "replay_target", conflicts_with_all = ["historical_key_file", "alchemy_config"])]
    historical_key: Option<String>,
    /// Read the provider credential from FILE without storing it in results.
    #[arg(long = "key-file", alias = "historical-key-file", value_name = "FILE", help_heading = help::PROVIDER, requires = "replay_target", conflicts_with = "alchemy_config")]
    historical_key_file: Option<PathBuf>,
}

impl Options {
    pub(in super::super) fn resolve(
        &self,
        legacy_file: Option<&std::path::Path>,
    ) -> Result<Option<ResolvedProvider>, Error> {
        let options = ProviderOptions {
            config: self.historical_config.clone(),
            provider: self.historical_provider.clone(),
            key: self.historical_key.clone(),
            key_file: self.historical_key_file.clone(),
        };
        let project = match std::fs::symlink_metadata("svm-replay.toml") {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => {
                return Err(Error::new(
                    "SOURCE_CONFIGURATION",
                    "cannot inspect svm-replay.toml",
                ))
            }
        };
        if options.is_explicit() || project {
            if legacy_file.is_some() {
                return Err(Error::new(
                    "SOURCE_CONFIGURATION",
                    "use --historical-key-file with historical provider configuration",
                ));
            }
            options.resolve().map(Some)
        } else {
            credentials::load(legacy_file)?;
            Ok(None)
        }
    }
}
