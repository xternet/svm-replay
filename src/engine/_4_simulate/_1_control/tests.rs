use super::*;

#[test]
fn failed_control_is_retained_without_claiming_verification() {
    let job = tempfile::tempdir().unwrap();
    let output = json!({"original":{"logs":["runtime mismatch"],"computeUnits":17}});
    let error = retain_failure(job.path(), &output, "metadata mismatch".into()).unwrap();
    assert_eq!(error.code, "MISMATCH");
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["unverifiedOutput"]["verified"], false);
    assert!(details.get("controlEvidence").is_none());
    let raw = std::fs::read(job.path().join("unverified-control-output.json")).unwrap();
    assert_eq!(
        details["unverifiedOutput"]["sha256"],
        json!(Digest::of(&raw))
    );
    assert_eq!(serde_json::from_slice::<Value>(&raw).unwrap(), output);
    let reference = details["unverifiedOutput"].clone();
    let receipt = crate::_5_finalize::job::finish_job(
        Err(error),
        json!({}),
        job.path(),
        std::time::Instant::now(),
    )
    .unwrap();
    assert_eq!(
        receipt["incomplete"]["available"]["unverifiedOutput"],
        reference
    );
    assert_eq!(receipt["incomplete"]["category"], "VERIFICATION_MISMATCH");
    assert!(receipt.get("output").is_none());
}
