use super::*;

/// Verify a benchmark case, retaining the historical replacement-change requirement.
pub fn verify(
    candidate: &Value,
    fixture: &Value,
    raw_block: &Value,
    output: &Value,
    metadata_policy: MetadataPolicy,
) -> Result<VerificationReport> {
    verify_with_options(candidate, fixture, raw_block, output, metadata_policy, true)
}
