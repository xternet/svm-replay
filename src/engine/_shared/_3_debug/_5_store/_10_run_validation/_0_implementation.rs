use super::*;

pub(in super::super) fn run_limit(name: &str, policy: &CaptureRequest) -> Result<usize, Error> {
    match name {
        "output.json" => Ok(MAX_FILE),
        "verification.json" | "control-evidence.json" | "events.json" | "receipt.json" => {
            Ok(16 * 1024 * 1024)
        }
        "original-control-artifact.json" | "requested-artifact.json" => Ok(MAX_MANIFEST),
        "original-control-capture.json" | "requested-capture.json" => {
            Ok(policy.limits.max_bytes as usize)
        }
        _ => Err(error(
            "SESSION_FORMAT",
            format!("unknown run artifact name: {name}"),
        )),
    }
}

pub(in super::super) fn run_names(manifest: &RunManifest) -> Result<BTreeSet<String>, Error> {
    require(
        manifest
            .exports
            .keys()
            .all(|phase| matches!(phase.as_str(), "original-control" | "requested")),
        "SESSION_FORMAT",
        "unknown export phase",
    )?;
    let mut names: BTreeSet<String> = [
        "output.json",
        "verification.json",
        "control-evidence.json",
        "events.json",
        "receipt.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for phase in manifest.exports.keys() {
        names.insert(format!("{phase}-artifact.json"));
        names.insert(format!("{phase}-capture.json"));
    }
    Ok(names)
}

pub(in super::super) fn run_blobs(
    manifest: &RunManifest,
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<(), Error> {
    require(
        manifest.schema == "svm-m17-saved-debug-run/v1"
            && manifest.status == "COMPLETE"
            && manifest.assurance == ASSURANCE,
        "SESSION_FORMAT",
        "unknown finalized-run schema/status/assurance",
    )?;
    manifest.bounds.validate(&manifest.policy)?;
    let names = run_names(manifest)?;
    require(
        manifest.files.keys().cloned().collect::<BTreeSet<_>>() == names
            && blobs.keys().cloned().collect::<BTreeSet<_>>() == names,
        "SESSION_FORMAT",
        "unexpected/missing finalized run files",
    )?;
    let mut total = 0usize;
    for (name, bytes) in blobs {
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| error("SESSION_LIMIT", "run size overflow"))?;
        require(
            bytes.len() <= run_limit(name, &manifest.policy)? && total <= MAX_BUNDLE,
            "SESSION_LIMIT",
            "run artifact byte budget exceeded",
        )?;
        require(
            manifest.files[name].bytes == bytes.len() as u64
                && manifest.files[name].sha256 == Digest::of(bytes),
            "SESSION_INTEGRITY",
            &format!("run artifact hash/size differs: {name}"),
        )?;
    }
    let receipt = parse_json(&blobs["receipt.json"])?;
    require(
        receipt["status"] == "PASS"
            && receipt["executionMode"] == manifest.policy.execution_mode
            && receipt["captureWorkerSha256"] == manifest.session.worker_sha256.as_str(),
        "SESSION_FORMAT",
        "run receipt disposition/mode/worker differs",
    )?;
    let events = parse_json(&blobs["events.json"])?;
    let events = events
        .as_array()
        .ok_or_else(|| error("SESSION_FORMAT", "run events are not an array"))?;
    require(
        events.len() <= 32768,
        "SESSION_LIMIT",
        "stored event count exceeds bound",
    )?;
    if manifest.policy.execution_mode == "interpreter-debug" {
        require(
            receipt["schema"] == "svm-m17-debug-execution/v1"
                && receipt["eventCount"].as_u64() == Some(events.len() as u64)
                && receipt["eventsSha256"] == json!(Digest::of(canonical_json(&json!(events)))),
            "SESSION_INTEGRITY",
            "live event receipt hash/count differs",
        )?;
    } else {
        require(
            receipt["schema"] == "svm-m17-trace-execution/v1",
            "SESSION_FORMAT",
            "JIT run has wrong receipt schema",
        )?;
    }
    let verification = parse_json(&blobs["verification.json"])?;
    let control = parse_json(&blobs["control-evidence.json"])?;
    require(
        verification["status"] == "PASS"
            && control["schema"] == "svm-original-control-evidence/v1"
            && control["complete"] == true
            && control["phase"] == "original-control"
            && control["verification"]["status"] == "PASS",
        "SESSION_FORMAT",
        "finalized run lacks separately passed control/requested verification",
    )?;
    for name in ["fixtureSha256", "outputSha256"] {
        Digest::new(
            control[name]
                .as_str()
                .ok_or_else(|| error("SESSION_FORMAT", "control proof hash missing"))?,
        )?;
    }
    let declared = receipt["exports"]
        .as_array()
        .ok_or_else(|| error("SESSION_FORMAT", "receipt export set missing"))?;
    require(
        declared.len() == manifest.exports.len(),
        "SESSION_INTEGRITY",
        "receipt/export set differs",
    )?;
    let mut phases = BTreeSet::new();
    for entry in declared {
        let phase = entry["phase"]
            .as_str()
            .ok_or_else(|| error("SESSION_FORMAT", "receipt export phase missing"))?;
        require(
            phases.insert(phase) && manifest.exports.contains_key(phase),
            "SESSION_INTEGRITY",
            "duplicate or unbound receipt export",
        )?;
        let artifact = parse_json(&blobs[&format!("{phase}-artifact.json")])?;
        require(
            entry["artifact"] == artifact,
            "SESSION_INTEGRITY",
            "receipt artifact differs",
        )?;
        crate::shared::trace::validate_artifact(
            &artifact,
            &manifest.policy,
            &manifest.exports[phase],
            Some(&blobs[&format!("{phase}-capture.json")]),
        )?;
    }
    Ok(())
}
