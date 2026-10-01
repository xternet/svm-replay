use super::_0_implementation::supports_recent_alt;

#[test]
fn recent_alt_gate_covers_reviewed_workers_but_not_unknown_executors() {
    for executor in [
        "litesvm-v0.6.1-agave-2.2.20",
        "litesvm-v0.7.1-agave-2.3.9",
        "litesvm-v0.8.2-agave-3.0.10",
        "litesvm-v0.12.0-agave-3.1.11",
        "litesvm-v0.13.1-agave-4.0.0",
        "litesvm-v0.14.0-pr402-agave-4.1.2",
        "litesvm-v0.16.0-agave-4.2.1",
    ] {
        assert!(supports_recent_alt(executor), "{executor}");
    }
    for executor in ["", "unreviewed", "litesvm-v0.16.0-agave-4.2.2"] {
        assert!(!supports_recent_alt(executor));
    }
}
