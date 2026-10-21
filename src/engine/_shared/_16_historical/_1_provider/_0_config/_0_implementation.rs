use super::super::_1_credentials;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};
use svm_replay_protocol::Error;

#[derive(Default)]
pub struct ProviderOptions {
    pub config: Option<PathBuf>,
    pub provider: Option<String>,
    pub key: Option<String>,
    pub key_file: Option<PathBuf>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Historical {
    provider: Option<String>,
    #[serde(default)]
    providers: BTreeMap<String, ProviderSpec>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct ProviderSpec {
    pub adapter: Option<String>,
    pub url: Option<String>,
    pub key_env: Option<String>,
    pub key_file: Option<PathBuf>,
}

/// Secrets deliberately have no serialization and no revealing Debug output.
pub struct ResolvedProvider {
    pub name: String,
    pub adapter: String,
    source_id: String,
    endpoint: String,
    credential: String,
}

impl fmt::Debug for ResolvedProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedProvider")
            .field("name", &self.name)
            .field("adapter", &self.adapter)
            .finish_non_exhaustive()
    }
}

impl ResolvedProvider {
    /// Cache identity excludes credentials but distinguishes configured endpoints.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub(crate) fn transport_parts(self) -> (String, String) {
        (self.endpoint, self.credential)
    }
}

pub(in super::super) fn invalid(message: &str) -> Error {
    Error::new("SOURCE_CONFIGURATION", message)
}

fn configuration(path: &Path) -> Result<Historical, Error> {
    let bytes = crate::shared::runtime::read_bounded_file(path, 64 * 1024)
        .map_err(|_| invalid("cannot read historical config (maximum 64 KiB)"))?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| invalid("historical config must be UTF-8"))?;
    // TOML diagnostics contain the offending line, which may contain a secret.
    let root: toml::Value =
        toml::from_str(text).map_err(|_| invalid("invalid historical TOML configuration"))?;
    let historical = root
        .get("historical")
        .ok_or_else(|| invalid("historical config requires a [historical] section"))?;
    historical.clone().try_into()
        .map_err(|_| invalid("invalid [historical] configuration; use provider, providers, adapter, url, key_env or key_file"))
}

impl ProviderOptions {
    pub fn is_explicit(&self) -> bool {
        self.config.is_some()
            || self.provider.is_some()
            || self.key.is_some()
            || self.key_file.is_some()
    }

    /// Project config is optional. An explicitly supplied path is never ignored.
    pub fn resolve(self) -> Result<ResolvedProvider, Error> {
        let path = match self.config {
            Some(path) => Some(path),
            None => match std::fs::symlink_metadata("svm-replay.toml") {
                Ok(_) => Some(PathBuf::from("svm-replay.toml")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => return Err(invalid("cannot inspect svm-replay.toml")),
            },
        };
        let mut config = match path.as_deref() {
            Some(path) => configuration(path)?,
            None => Historical::default(),
        };
        let name = self
            .provider
            .or(config.provider)
            .unwrap_or_else(|| "alchemy".into());
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(invalid("invalid historical provider name"));
        }
        let spec = match config.providers.remove(&name) {
            Some(spec) => spec,
            None if name == "alchemy" => ProviderSpec::default(),
            None => {
                return Err(invalid(&format!(
                    "historical provider: {name:?} is not configured"
                )))
            }
        };
        let adapter = match &spec.adapter {
            Some(adapter) => adapter.as_str(),
            None if name == "alchemy" => "alchemy",
            None => {
                return Err(invalid(
                    "named historical provider requires adapter = 'alchemy'",
                ))
            }
        };
        if adapter != "alchemy" {
            return Err(invalid(
                "unsupported historical adapter; only alchemy is implemented",
            ));
        }
        let root = path
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."));
        let credential = _1_credentials::resolve(&spec, self.key, self.key_file, root)?;
        let default_url = "https://solana-mainnet.g.alchemy.com/v2/{key}";
        let template = spec.url.as_deref().unwrap_or(default_url);
        if template.matches("{key}").count() != 1 {
            return Err(invalid(
                "Alchemy url must contain exactly one {key} placeholder",
            ));
        }
        let endpoint = template.replace("{key}", &credential);
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|_| invalid("invalid historical provider URL"))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid(
                "historical provider URL requires HTTPS, no user info or fragment",
            ));
        }
        let source_id = if name == "alchemy" && template == default_url {
            "alchemy-mainnet-archive".into()
        } else {
            let pin = svm_replay_protocol::Digest::of(template.as_bytes());
            format!("alchemy-{name}-{}", pin.as_str())
        };
        Ok(ResolvedProvider {
            name,
            adapter: adapter.into(),
            source_id,
            endpoint,
            credential,
        })
    }
}
