use super::super::_0_config::{invalid, ProviderSpec};
use std::path::{Path, PathBuf};
use svm_replay_protocol::Error;

fn key_file(path: &Path) -> Result<String, Error> {
    let bytes = crate::shared::runtime::read_bounded_file(path, 1024)
        .map_err(|_| invalid("cannot read historical key file (maximum 1 KiB)"))?;
    std::str::from_utf8(&bytes)
        .map(|text| text.trim().to_owned())
        .map_err(|_| invalid("historical key file must be UTF-8"))
}

fn environment(name: &str) -> Result<String, Error> {
    if name.is_empty()
        || name.starts_with(|c: char| c.is_ascii_digit())
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(invalid("key_env must be an environment variable name"));
    }
    std::env::var(name)
        .map_err(|_| invalid("historical credential environment variable is missing or not UTF-8"))
}

pub(in super::super) fn resolve(
    spec: &ProviderSpec,
    explicit_key: Option<String>,
    explicit_file: Option<PathBuf>,
    config_root: &Path,
) -> Result<String, Error> {
    if explicit_key.is_some() && explicit_file.is_some() {
        return Err(invalid("choose historical key or key file, not both"));
    }
    if spec.key_env.is_some() && spec.key_file.is_some() {
        return Err(invalid("configure key_env or key_file, not both"));
    }
    let key = match (explicit_key, explicit_file) {
        (Some(key), None) => match key.strip_prefix("env:") {
            Some(name) => environment(name)?,
            None => key,
        },
        (None, Some(path)) => key_file(&path)?,
        (None, None) => match (&spec.key_env, &spec.key_file) {
            (Some(name), None) => environment(name)?,
            (None, Some(path)) => key_file(&config_root.join(path))?,
            (None, None) => environment("API_ALCHEMY")?,
            _ => return Err(invalid("ambiguous historical credentials")),
        },
        _ => return Err(invalid("ambiguous historical credentials")),
    };
    if key.is_empty()
        || key.len() > 512
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid(
            "historical key must be a nonempty API key, not a URL or expression",
        ));
    }
    Ok(key)
}
