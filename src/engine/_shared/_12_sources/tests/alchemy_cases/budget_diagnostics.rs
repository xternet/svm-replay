use super::*;

#[test]
fn exhausted_budget_identifies_counter_and_limit() {
    for (name, budget) in [
        (
            "requests",
            SourceLimits {
                max_requests: 1,
                ..limits()
            },
        ),
        (
            "accountReads",
            SourceLimits {
                max_account_reads: 1,
                ..limits()
            },
        ),
    ] {
        let source = AlchemySource::with_transport(
            config(),
            budget,
            Transport::values(vec![genesis(), account()]),
        )
        .unwrap();
        let error = match source.inspect(&query(2)) {
            Ok(_) => source.inspect(&query(3)).unwrap_err(),
            Err(error) => error,
        };
        assert_eq!(error.code, "SOURCE_RESOURCE_LIMIT");
        let value = serde_json::to_value(error).unwrap();
        assert_eq!(value["details"]["reason"], "BUDGET_EXCEEDED");
        assert_eq!(value["details"]["limitName"], name);
        assert_eq!(value["details"]["used"], 1);
        assert_eq!(value["details"]["limit"], 1);
    }
}
