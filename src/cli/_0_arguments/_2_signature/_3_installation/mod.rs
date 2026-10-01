use serde::Deserialize;
use std::path::PathBuf;
use svm_replay_engine::shared::runtime::{file_sha256, read_bounded_file};
use svm_replay_protocol::{parse_json, Digest, Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Installation {
    schema: String,
    bundle_sha256: Digest,
}

pub(in crate::_0_arguments) fn catalog() -> Result<(PathBuf, Digest), Error> {
    let executable = std::env::current_exe().map_err(|e| Error::new("BUNDLE_IO", e.to_string()))?;
    let root = executable
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| {
            Error::new(
                "NOT_INSTALLED",
                "install the native SVM Replay bundle first",
            )
        })?;
    let bytes = read_bounded_file(&root.join("installation.json"), 4096).map_err(|_| {
        Error::new(
            "NOT_INSTALLED",
            "run svm-replay from an installed native bundle",
        )
    })?;
    let pin: Installation = serde_json::from_value(parse_json(&bytes)?)
        .map_err(|e| Error::new("BUNDLE_FORMAT", e.to_string()))?;
    if pin.schema != "svm-replay-installation-pin/v1" {
        return Err(Error::new("BUNDLE_FORMAT", "unknown installation schema"));
    }
    let installed = crate::bundle::open(&root.join("bundle.json"), &pin.bundle_sha256)?;
    if file_sha256(&executable).map_err(|e| Error::new("BUNDLE_IO", e.to_string()))?
        != installed.binary_sha256.as_str()
    {
        return Err(Error::new(
            "BUNDLE_IDENTITY",
            "installed executable differs from its pin",
        ));
    }
    Ok((installed.catalog, installed.catalog_sha256))
}
