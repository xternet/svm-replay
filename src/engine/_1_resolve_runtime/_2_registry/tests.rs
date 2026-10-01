use super::*;
use serde_json::json;
use svm_replay_protocol::{worker::WorkerDescriptor, Digest};

fn worker(registry: &Registry, slot: u64) -> WorkerDescriptor {
    let profile = registry.profile(slot).unwrap();
    let executor = registry.executor(&profile.executor_source_id).unwrap();
    WorkerDescriptor {
        family: executor["packageName"].as_str().unwrap().into(),
        file: "unused-test-worker".into(),
        sha256: Digest::of(b"test worker"),
        build_hash: Digest::of(b"test build"),
        source_sha256: Digest::new(executor["reviewedSourceSha256"].as_str().unwrap()).unwrap(),
        executor_source_id: profile.executor_source_id.clone(),
        capabilities: vec!["guarded-generic-sysvars/v1".into()],
    }
}

#[test]
fn every_interval_binds_both_edges_and_rejects_wrong_worker() {
    let registry = Registry::bundled().unwrap();
    for profile in &registry.profiles {
        for slot in [profile.earliest_slot, profile.latest_slot] {
            let mut worker = worker(&registry, slot);
            let binding = registry.bind(slot, &worker).unwrap();
            svm_replay_protocol::runtime::validate_runtime_binding(&binding).unwrap();
            assert_eq!(binding["targetSlot"], slot);
            assert_eq!(binding["runtimeProfileId"], json!(profile.profile_id));
            worker.source_sha256 = Digest::of(b"wrong source");
            assert!(registry.bind(slot, &worker).is_err());
        }
    }
    assert_eq!(
        registry
            .profile(registry.window.earliest_slot - 1)
            .unwrap_err()
            .code,
        "UNSUPPORTED_RUNTIME"
    );
    assert_eq!(
        registry
            .profile(registry.window.latest_slot + 1)
            .unwrap_err()
            .code,
        "UNSUPPORTED_RUNTIME"
    );
}

#[test]
fn feature_activation_changes_at_exact_slot() {
    let registry = Registry::bundled().unwrap();
    let feature = registry
        .features
        .iter()
        .find(|f| {
            f["activationSlot"].as_u64().is_some_and(|s| {
                s > registry.window.earliest_slot && s < registry.window.latest_slot
            })
        })
        .unwrap();
    let slot = feature["activationSlot"].as_u64().unwrap();
    let before = registry
        .bind(slot - 1, &worker(&registry, slot - 1))
        .unwrap();
    let after = registry.bind(slot, &worker(&registry, slot)).unwrap();
    assert!(!before["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == feature["id"]));
    assert!(after["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == feature["id"]));
}

#[test]
fn bundled_registry_includes_qualified_early_runtime_and_transition() {
    let registry = Registry::bundled().unwrap();
    let earliest = worker(&registry, registry.window.earliest_slot);
    assert_eq!(earliest.family, "v2-2");
    assert_eq!(earliest.executor_source_id, "litesvm-v0.6.1-agave-2.2.20");
    // Reviewed leader-schedule activation: transition must use the newer executor.
    let transition = worker(&registry, 363_312_000);
    assert_eq!(transition.family, "v2-3");
    registry.bind(363_312_000, &transition).unwrap();
}

#[test]
fn declared_registry_gap_is_unsupported_not_an_executor_fallback() {
    let mut value: Value = serde_json::from_slice(include_bytes!("catalog.json")).unwrap();
    let first = value["profiles"][0].clone();
    let start = first["earliestSlot"].as_u64().unwrap();
    let end = first["latestSlot"].as_u64().unwrap();
    assert!(end > start + 4);
    let mut left = first.clone();
    let mut right = first;
    left["latestSlot"] = json!(start + 1);
    right["earliestSlot"] = json!(start + 3);
    value["profiles"] = json!([left, right]);
    value["window"]["latestSlot"] = json!(end);
    let parse = |v: &Value| Registry::from_bytes(&serde_json::to_vec(v).unwrap());
    assert!(
        parse(&value).is_err(),
        "accidental profile loss must reject"
    );
    value["unsupportedIntervals"] = json!([{
        "earliestSlot": start + 2, "latestSlot": start + 2
    }]);
    let registry = parse(&value).unwrap();
    for slot in [start, start + 1, start + 3, end] {
        registry.bind(slot, &worker(&registry, slot)).unwrap();
    }
    assert_eq!(
        registry.profile(start + 2).unwrap_err().code,
        "UNSUPPORTED_RUNTIME"
    );
    let valid = value.clone();
    for gaps in [
        json!([]),
        json!([{"earliestSlot": start + 1, "latestSlot": start + 2}]),
        json!([{"earliestSlot": start + 2, "latestSlot": start + 3}]),
        json!([{"earliestSlot": start + 2, "latestSlot": start + 1}]),
        json!([{"earliestSlot": start + 2, "latestSlot": start + 2},
               {"earliestSlot": end + 1, "latestSlot": end + 1}]),
    ] {
        value = valid.clone();
        value["unsupportedIntervals"] = gaps;
        assert!(parse(&value).is_err(), "incorrect declared gap accepted");
    }
    value = valid;
    value["profiles"][1]["earliestSlot"] = json!(start + 1);
    assert!(parse(&value).is_err(), "overlapping profiles accepted");
}
