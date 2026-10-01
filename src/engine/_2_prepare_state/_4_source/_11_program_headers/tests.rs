use super::*;

#[test]
fn anchor_search_is_bounded_by_reviewed_native_versions() {
    let old = "litesvm-v0.6.1-agave-2.2.20";
    let registry = json!({"profiles":[{"earliestSlot":1,"latestSlot":100,"executorSourceId":old},
        {"earliestSlot":101,"latestSlot":200,"executorSourceId":"unknown"}]});
    assert_eq!(anchor_slots(&registry, 50, old).unwrap(), vec![100]);
    assert!(anchor_slots(&registry, 101, "unknown").is_err());
    assert!(anchor_slots(&registry, 50, "wrong").is_err());
}

#[test]
fn same_slot_creation_is_not_mistaken_for_an_archive_gap() {
    let tx = |balance| {
        json!({"transaction":{"message":{"accountKeys":["program"]}},
        "meta":{"preBalances":[balance]}})
    };
    assert!(!preexisting(&json!({"transactions":[tx(0),tx(99)]}), "program").unwrap());
    assert!(preexisting(&json!({"transactions":[tx(99)]}), "program").unwrap());
    assert!(!preexisting(&json!({"transactions":[tx(99)]}), "other").unwrap());
    assert!(preexisting(
        &json!({"transactions":[{"transaction":{"message":{"accountKeys":["program"]}},
        "meta":{"preBalances":[]}}]}),
        "program"
    )
    .is_err());
}
