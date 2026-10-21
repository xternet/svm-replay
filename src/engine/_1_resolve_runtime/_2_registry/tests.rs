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
}

#[test]
fn newer_slots_try_latest_runtime_without_claiming_reviewed_coverage() {
    let registry = Registry::bundled().unwrap();
    let last = registry.window.latest_slot;
    let latest = registry.profile(last).unwrap();
    for slot in [last + 1, 457_797_318, 9_007_199_254_740_991] {
        let profile = registry.profile(slot).unwrap();
        assert_eq!(profile.executor_source_id, latest.executor_source_id);
        assert_eq!(
            profile.latest_slot, last,
            "reviewed coverage must not expand"
        );
        let binding = registry.bind(slot, &worker(&registry, slot)).unwrap();
        svm_replay_protocol::runtime::validate_runtime_binding(&binding).unwrap();
        let selection = &binding["executor"]["runtimeSelection"];
        assert_eq!(selection["status"], "UNQUALIFIED_NEWER_SLOT");
        assert_eq!(selection["reviewedThroughSlot"], last);
        assert_eq!(selection["targetSlot"], slot);
        assert!(selection["warning"]
            .as_str()
            .unwrap()
            .contains("last-known features"));
    }
    let reviewed = registry.bind(last, &worker(&registry, last)).unwrap();
    assert!(reviewed["executor"].get("runtimeSelection").is_none());
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

#[test]
fn every_pinned_runtime_has_its_official_agave_name() {
    let registry = Registry::bundled().unwrap();
    let names: Vec<_> = (registry.runtime_names().unwrap().into_iter())
        .map(|n| (n.family.clone(), n.name(), n.litesvm.clone()))
        .collect();
    let expected = [
        ("v2-2", "Agave 2.2.20", "0.6.1"),
        ("v2-3", "Agave 2.3.9", "0.7.1"),
        ("v3-0", "Agave 3.0.10", "0.8.2"),
        ("v3-1", "Agave 3.1.11", "0.12.0"),
        ("v4-0", "Agave 4.0.0", "0.13.1"),
        ("v4-1", "Agave 4.1.2", "0.14.0+pr402"),
        ("v4-2", "Agave 4.2.1", "0.16.0"),
    ];
    let expected: Vec<_> = (expected.iter())
        .map(|(f, n, l)| (f.to_string(), n.to_string(), l.to_string()))
        .collect();
    assert_eq!(names, expected);
    // Family IDs stay the machine IDs; reviewed slots cover the window without overlap.
    let mut next = registry.window.earliest_slot;
    for name in registry.runtime_names().unwrap() {
        assert_eq!(registry.runtime_name_of_family(&name.family).unwrap(), name);
        for (first, last) in &name.reviewed_slots {
            assert_eq!(*first, next);
            next = last + 1;
        }
    }
    assert_eq!(next, registry.window.latest_slot + 1);
}

#[test]
fn unknown_or_inconsistent_runtimes_are_errors_not_guesses() {
    let registry = Registry::bundled().unwrap();
    let unknown = registry
        .runtime_name("litesvm-v9.9.9-agave-9.9.9")
        .unwrap_err();
    assert_eq!(unknown.code, "UNSUPPORTED_RUNTIME");
    assert_eq!(
        registry.runtime_name_of_family("v9-9").unwrap_err().code,
        "UNSUPPORTED_RUNTIME"
    );
    let mut catalog: serde_json::Value =
        serde_json::from_slice(include_bytes!("catalog.json")).unwrap();
    catalog["executors"][0]["agaveFeatureSetVersion"] = json!("9.9.9");
    let altered = Registry::from_bytes(&serde_json::to_vec(&catalog).unwrap()).unwrap();
    let id = catalog["executors"][0]["id"].as_str().unwrap();
    assert_eq!(
        altered.runtime_name(id).unwrap_err().code,
        "RUNTIME_REGISTRY"
    );
    // Workers: an explicit executor outside the registry is unnamed; a pinned one must match its family.
    let mut explicit = worker(&registry, registry.window.earliest_slot);
    explicit.executor_source_id = "explicit-test-executor".into();
    assert_eq!(
        registry.worker_runtime_json(&explicit).unwrap(),
        json!(null)
    );
    let mut mislabeled = worker(&registry, registry.window.earliest_slot);
    mislabeled.family = "v4-2".into();
    assert_eq!(
        registry.worker_runtime(&mislabeled).unwrap_err().code,
        "RUNTIME_REGISTRY"
    );
    let pinned = worker(&registry, registry.window.earliest_slot);
    assert_eq!(
        registry.worker_runtime_json(&pinned).unwrap()["name"],
        "Agave 2.2.20"
    );
}
