use super::*;
use svm_replay_engine::shared::{
    historical::{After, SessionHydration, SessionInput},
    runtime::{file_sha256, CancellationToken},
};

pub(crate) struct SessionPreparation {
    pub after: String,
    pub data_dir: PathBuf,
    pub historical: HistoricalOptions,
    pub controls: SignatureControls,
    pub runtime: super::super::super::RuntimeArgs,
}

pub(crate) fn prepare_session(
    input: SessionPreparation,
    cancellation: CancellationToken,
) -> Result<(SessionInput, SessionHydration), Error> {
    let (tx, after, position_after) = match input.after.parse::<After>()? {
        After::Transaction(signature) => (Some(signature), None, true),
        After::Slot(slot) => (None, Some(format!("slot:{slot}")), false),
    };
    if input.controls.collect.is_some() {
        return Err(Error::new(
            "INVALID_REQUEST",
            "session preparation does not accept --collect",
        ));
    }
    let options = Options {
        tx,
        after,
        data_dir: Some(input.data_dir),
        historical: input.historical,
        controls: input.controls,
        runtime: input.runtime,
        ..Options::default()
    };
    let (receipt, hydration, catalog) = execution::execute_for_session(&options, cancellation)?;
    if receipt["outcome"] != "COMPLETED" {
        let code = receipt["error"]["code"].as_str().ok_or_else(|| {
            Error::new(
                "SESSION_PREPARATION",
                "preparation failure lacks a typed error",
            )
        })?;
        let message = receipt["error"]["message"].as_str().ok_or_else(|| {
            Error::new(
                "SESSION_PREPARATION",
                "preparation failure lacks an error message",
            )
        })?;
        return Err(Error::new(code, message).with_details(receipt));
    }
    if receipt["verification"]["status"] != "PASS"
        && !(receipt["verification"]["status"] == "INITIALIZED"
            && receipt["originalControl"]["status"] == "NOT_APPLICABLE")
    {
        return Err(Error::new(
            "SESSION_PREPARATION",
            "historical original-control preparation did not pass",
        )
        .with_details(receipt));
    }
    let path = receipt["receiptPath"]
        .as_str()
        .ok_or_else(|| Error::new("SESSION_PREPARATION", "retained receipt path missing"))?;
    let descriptor = catalog
        .manifest
        .workers
        .iter()
        .find(|worker| receipt["worker"]["family"] == worker.family)
        .ok_or_else(|| {
            Error::new(
                "WORKER_IDENTITY",
                "prepared runtime missing from installed catalog",
            )
        })?;
    Ok((
        SessionInput {
            schema: "svm-historical-session-input/v1".into(),
            receipt_path: PathBuf::from(path),
            receipt_sha256: file_sha256(std::path::Path::new(path))
                .map_err(|error| Error::new("SESSION_PREPARATION", error.to_string()))?,
            worker_path: catalog.root.join(&descriptor.file),
            after: position_after,
        },
        hydration,
    ))
}
