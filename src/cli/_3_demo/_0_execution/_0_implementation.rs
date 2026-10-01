use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct Installation {
    pub(in super::super) schema: String,
    pub(in super::super) bundle_sha256: Digest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct Demo {
    pub(in super::super) schema: String,
    pub(in super::super) case_id: String,
    pub(in super::super) request_sha256: Digest,
    pub(in super::super) expected_output_sha256: Digest,
}

pub(in super::super) fn read(path: &std::path::Path, max: usize) -> Result<Vec<u8>, Error> {
    read_bounded_file(path, max).map_err(|error| Error::new("DEMO_INPUT", error.to_string()))
}

pub fn run(
    bundle: Option<PathBuf>,
    pin: Option<String>,
    data_dir: PathBuf,
) -> Result<Value, Error> {
    let loading = super::super::super::terminal::Progress::start(
        "Checking installed bundle and cached inputs",
    );
    let executable = std::env::current_exe().map_err(|e| Error::new("DEMO_IO", e.to_string()))?;
    let (path, pin) = match (bundle, pin) {
        (Some(path), Some(pin)) => (path, Digest::new(pin)?),
        (None, None) => {
            let root = executable
                .parent()
                .and_then(|p| p.parent())
                .ok_or_else(|| {
                    Error::new("DEMO_NOT_INSTALLED", "installation root is unavailable")
                })?;
            let installation = root.join("installation.json");
            match std::fs::symlink_metadata(&installation) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(Error::new(
                        "DEMO_NOT_INSTALLED",
                        "install an approved demo bundle first, or supply --bundle and --bundle-sha256",
                    ));
                }
                Err(error) => return Err(Error::new("DEMO_INPUT", error.to_string())),
            }
            let bytes = read(&installation, 4096)?;
            let installed: Installation = serde_json::from_value(parse_json(&bytes)?)
                .map_err(|e| Error::new("DEMO_INPUT", e.to_string()))?;
            if installed.schema != "svm-replay-installation-pin/v1" {
                return Err(Error::new("DEMO_INPUT", "unknown installation schema"));
            }
            (root.join("bundle.json"), installed.bundle_sha256)
        }
        _ => {
            return Err(Error::new(
                "DEMO_INPUT",
                "bundle and pin must be supplied together",
            ))
        }
    };
    let installed = super::super::super::bundle::open(&path, &pin)?;
    let sha256 = file_sha256(&executable).map_err(|e| Error::new("DEMO_IO", e.to_string()))?;
    if sha256 != installed.binary_sha256.as_str() {
        return Err(Error::new(
            "BUNDLE_IDENTITY",
            "run the CLI included in this bundle",
        ));
    }
    let (manifest, request_path) = super::super::super::bundle::demo_files(&path, &pin)?;
    let demo: Demo = serde_json::from_value(parse_json(&read(&manifest, 4096)?)?)
        .map_err(|e| Error::new("DEMO_INPUT", e.to_string()))?;
    if demo.schema != "svm-replay-demo/v1" || demo.case_id.is_empty() {
        return Err(Error::new("DEMO_INPUT", "unknown or empty demo manifest"));
    }
    let bytes = read(&request_path, 256 * 1024 * 1024)?;
    if Digest::of(&bytes) != demo.request_sha256 {
        return Err(Error::new("DEMO_INTEGRITY", "demo request pin differs"));
    }
    let request = PreparedRequest::parse(&bytes)?;
    loading.finish("Bundle and cached inputs verified");
    super::super::super::terminal::section("Selected example");
    super::super::super::terminal::stage(&format!(
        "Case: {} | Runtime: {}",
        demo.case_id, request.family
    ));
    for (label, value) in [
        ("Transaction", &request.fixture["target"]["signature"]),
        (
            "Slot",
            &request.fixture["runtime"]["bankContext"]["targetSlot"],
        ),
    ] {
        if !value.is_null() {
            super::super::super::terminal::stage(&format!(
                "{label}: {}",
                crate::terminal::display(value)
            ));
        }
    }
    super::super::super::terminal::stage(
        "Source: bundled historical inputs. Offline execution; no API key, wallet or broadcast.",
    );
    let config = Config {
        catalog_path: installed.catalog,
        catalog_sha256: installed.catalog_sha256,
        data_dir,
        owner: ProcessOwner { executable, sha256 },
        cache: CacheMode::Off,
        trace: None,
    };
    super::super::super::terminal::section("Replay");
    let replaying =
        super::super::super::terminal::Progress::start("Executing historical replay through SVM");
    let receipt = simulate_prepared(
        request,
        &config,
        super::super::super::lifecycle::cancellation::install()?,
    )?;
    if receipt["outcome"] == "COMPLETED" {
        replaying.finish("Historical replay completed");
    } else {
        drop(replaying);
    }
    super::super::super::terminal::stage("Checking result against the pinned historical baseline");
    if receipt["outcome"] != "COMPLETED" {
        return Ok(receipt);
    }
    if receipt["output"]["sha256"].as_str() != Some(demo.expected_output_sha256.as_str())
        || receipt["verification"]["status"] != "PASS"
        || receipt["controlVerification"]["status"] != "PASS"
        || receipt["currentStateFallback"] != false
    {
        return Err(Error::new(
            "DEMO_MISMATCH",
            "fresh replay did not match the approved demo baseline",
        )
        .with_details(
            json!({"receiptPath":receipt["receiptPath"],"actualOutput":receipt["output"]}),
        ));
    }
    Ok(
        json!({"schema":"svm-replay-demo-result/v1", "outcome":"COMPLETED",
        "caseId":demo.case_id,"matched":true,"execution":"fresh-offline-supplied-state",
        "receiptPath":receipt["receiptPath"],"receipt":receipt}),
    )
}
