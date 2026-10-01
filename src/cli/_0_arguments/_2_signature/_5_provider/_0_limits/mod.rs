use super::{Controls, Duration, SourceLimits};

pub(super) fn source_limits(controls: &Controls, remaining: Duration) -> SourceLimits {
    let max_requests = controls.max_requests.unwrap_or(5000);
    SourceLimits {
        max_requests,
        max_account_reads: max_requests,
        max_download_bytes: controls.max_download_bytes.unwrap_or(512 * 1024 * 1024),
        max_response_bytes: 32 * 1024 * 1024,
        deadline: remaining,
    }
}

#[cfg(test)]
mod tests;
