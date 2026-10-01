use super::*;

#[test]
fn signature_request_budget_also_controls_account_reads() {
    for maximum in [None, Some(10_000)] {
        let controls = Controls {
            max_requests: maximum,
            ..Controls::default()
        };
        let limits = source_limits(&controls, Duration::from_secs(300));
        assert_eq!(limits.max_account_reads, limits.max_requests);
        assert_eq!(limits.max_requests, maximum.unwrap_or(5000));
    }
}
