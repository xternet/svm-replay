use super::*;

pub fn save_run(
    directory: &Path,
    session: &SavedSession,
    artifacts: &RunArtifacts<'_>,
) -> Result<Digest, Error> {
    // Reopen the externally pinned session before accepting run association.
    let verified = open_session(&session.directory, &session.pin)?;
    require(
        verified.manifest.session_identity == session.manifest.session_identity,
        "SESSION_INTEGRITY",
        "supplied saved-session handle differs from pinned bundle",
    )?;
    require(
        artifacts.receipt["referenceSha256"] == verified.manifest.reference.sha256.as_str()
            && artifacts.receipt["gateSha256"] == verified.manifest.capture.gate_sha256.as_str()
            && artifacts.receipt["implementationSha256"]
                == verified.manifest.implementation_sha256.as_str(),
        "SESSION_IDENTITY",
        "run provenance differs from saved session",
    )?;
    require(
        artifacts.control_evidence["fixtureSha256"]
            == json!(Digest::of(canonical_json(&verified.control.fixture))),
        "SESSION_LINEAGE",
        "run lost immutable original control",
    )?;
    let has_variant = encoded(&verified.request)? != encoded(&verified.control)?;
    if !has_variant {
        require(
            artifacts.control_evidence["outputSha256"]
                == json!(Digest::of(canonical_json(artifacts.output))),
            "SESSION_INTEGRITY",
            "original-only result differs from control proof",
        )?;
    }
    let expected_phases: BTreeSet<&str> = if verified.manifest.policy.level == "off" {
        BTreeSet::new()
    } else if has_variant {
        ["original-control", "requested"].into_iter().collect()
    } else {
        ["original-control"].into_iter().collect()
    };
    require(
        artifacts
            .exports
            .iter()
            .map(|(phase, _)| phase.as_str())
            .collect::<BTreeSet<_>>()
            == expected_phases
            && artifacts.exports.len() == expected_phases.len(),
        "SESSION_FORMAT",
        "run export phases differ from saved variant policy",
    )?;
    let mut blobs = BTreeMap::new();
    for (name, value) in [
        ("output.json", artifacts.output),
        ("verification.json", artifacts.verification),
        ("control-evidence.json", artifacts.control_evidence),
        ("receipt.json", artifacts.receipt),
    ] {
        blobs.insert(name.into(), encoded(value)?);
    }
    blobs.insert("events.json".into(), encoded(&artifacts.events)?);
    let mut identities = BTreeMap::new();
    for (phase, export) in artifacts.exports {
        let fixture = if phase == "original-control" {
            &verified.control.fixture
        } else {
            &verified.request.fixture
        };
        let key = capture_identity(
            &Digest::of(canonical_json(fixture)),
            &verified.manifest.capture.worker,
            &verified.manifest.policy,
            &verified.manifest.bounds,
        )?;
        let schema = if verified.manifest.policy.execution_mode == "interpreter-debug" {
            "svm-m17-debug-identity/v1"
        } else {
            "svm-m17-gated-capture/v1"
        };
        let identity = Digest::of(canonical_json(
            &json!({"schema":schema,"captureIdentity":key,"gateSha256":verified.manifest.capture.gate_sha256,"referenceSha256":verified.manifest.capture.reference_sha256,"implementationSha256":verified.manifest.implementation_sha256,"phase":phase,"blockSha256":verified.request.block_sha256,"candidate":verified.request.candidate,"sourceEvidenceHashes":verified.request.source_evidence_hashes}),
        ));
        crate::shared::trace::validate_artifact(
            &export.artifact,
            &verified.manifest.policy,
            &identity,
            Some(&export.payload),
        )?;
        identities.insert(phase.clone(), identity);
        blobs.insert(format!("{phase}-artifact.json"), encoded(&export.artifact)?);
        blobs.insert(format!("{phase}-capture.json"), export.payload.clone());
    }
    let manifest = RunManifest {
        schema: "svm-m17-saved-debug-run/v1".into(),
        status: "COMPLETE".into(),
        assurance: ASSURANCE.into(),
        session: verified.pin,
        session_identity: verified.manifest.session_identity,
        policy: verified.manifest.policy,
        bounds: verified.manifest.bounds,
        exports: identities,
        files: blobs
            .iter()
            .map(|(name, bytes)| {
                (
                    name.clone(),
                    FileReference {
                        sha256: Digest::of(bytes),
                        bytes: bytes.len() as u64,
                    },
                )
            })
            .collect(),
    };
    run_blobs(&manifest, &blobs)?;
    let bytes = encoded(&manifest)?;
    require(
        bytes.len() <= MAX_MANIFEST && directory.is_absolute(),
        "SESSION_LIMIT",
        "manifest size/destination bound",
    )?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory).map_err(|e| {
        error(
            "SESSION_IO",
            format!("create-only run {}: {e}", directory.display()),
        )
    })?;
    let result = (|| {
        for (name, bytes) in &blobs {
            write_file(&directory.join(name), bytes, false)?;
        }
        write_file(&directory.join("run.json"), &bytes, false)?;
        sync_directory(directory)?;
        Ok(Digest::of(&bytes))
    })();
    result.map_err(|error: Error| error.with_details(json!({"partialDirectoryRetained":directory})))
}
