use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub earliest_slot: u64,
    pub latest_slot: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub earliest_slot: u64,
    pub latest_slot: u64,
    pub executor_source_id: String,
    pub profile_id: Digest,
}

#[derive(Deserialize)]
pub struct Registry {
    pub window: Window,
    #[serde(default, rename = "unsupportedIntervals")]
    unsupported_intervals: Vec<Window>,
    pub(in super::super) profiles: Vec<Profile>,
    pub(in super::super) features: Vec<Value>,
    pub(in super::super) builtins: Vec<Value>,
    pub(in super::super) precompiles: Vec<Value>,
    pub(in super::super) executors: Vec<Value>,
}

pub(in super::super) fn invalid(message: &str) -> Error {
    Error::new("RUNTIME_REGISTRY", message)
}

impl Registry {
    /// Frozen metadata for native lifecycle proofs, not an execution override.
    pub(crate) fn lifecycle_catalog(&self) -> Value {
        json!({"window":{"earliestSlot":self.window.earliest_slot,"latestSlot":self.window.latest_slot},
            "features":self.features,"builtins":self.builtins,"precompiles":self.precompiles,
            "profiles":self.profiles.iter().map(|p| json!({
                "earliestSlot":p.earliest_slot,"latestSlot":p.latest_slot,
                "executorSourceId":p.executor_source_id,"profileId":p.profile_id})).collect::<Vec<_>>()})
    }
    pub fn bundled() -> Result<Self, Error> {
        Self::from_bytes(include_bytes!("../catalog.json"))
    }
    /// Explicit reviewed registry; callers must independently verify its trusted pin.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let registry: Self = serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
        let mut next = registry.window.earliest_slot;
        let mut gaps = registry.unsupported_intervals.iter();
        for profile in &registry.profiles {
            if profile.earliest_slot > next {
                let gap = gaps
                    .next()
                    .ok_or_else(|| invalid("undeclared profile gap"))?;
                if gap.earliest_slot != next
                    || gap.latest_slot < next
                    || gap.latest_slot.checked_add(1) != Some(profile.earliest_slot)
                {
                    return Err(invalid("unsupported interval does not match profile gap"));
                }
                next = profile.earliest_slot;
            }
            if profile.earliest_slot != next || profile.latest_slot < next {
                return Err(invalid("profile ranges overlap or are reversed"));
            }
            registry.executor(&profile.executor_source_id)?;
            next = profile
                .latest_slot
                .checked_add(1)
                .ok_or_else(|| invalid("slot overflow"))?;
        }
        if gaps.next().is_some()
            || registry.profiles.is_empty()
            || next.checked_sub(1) != Some(registry.window.latest_slot)
        {
            return Err(invalid("profile coverage differs from reviewed window"));
        }
        Ok(registry)
    }
    pub fn profile(&self, slot: u64) -> Result<&Profile, Error> {
        self.profiles
            .iter()
            .find(|p| slot >= p.earliest_slot && slot <= p.latest_slot)
            .ok_or_else(|| {
                Error::new(
                    "UNSUPPORTED_RUNTIME",
                    format!(
                        "slot {slot} has no reviewed runtime profile (registry bounds {}..={})",
                        self.window.earliest_slot, self.window.latest_slot
                    ),
                )
            })
    }
    pub(in super::super) fn executor(&self, id: &str) -> Result<&Value, Error> {
        self.executors
            .iter()
            .find(|e| e["id"] == id)
            .ok_or_else(|| invalid("profile executor is missing"))
    }
    pub fn bind(&self, slot: u64, worker: &WorkerDescriptor) -> Result<Value, Error> {
        let profile = self.profile(slot)?;
        let source = self.executor(&profile.executor_source_id)?;
        if worker.executor_source_id != profile.executor_source_id
            || source["packageName"] != worker.family
            || source["reviewedSourceSha256"] != json!(worker.source_sha256)
        {
            return Err(Error::new(
                "WORKER_IDENTITY",
                "installed source differs from reviewed profile",
            ));
        }
        let mut executor = source.clone();
        let object = executor
            .as_object_mut()
            .ok_or_else(|| invalid("executor shape"))?;
        object.remove("packageName");
        object.remove("reviewedSourceSha256");
        executor["sourceEvidenceHash"] = json!(worker.source_sha256);
        executor["m9Build"] = json!({"binarySha256":worker.sha256,
            "buildHash":worker.build_hash,"capabilities":worker.capabilities});
        let mut features = self
            .features
            .iter()
            .filter(|f| {
                f["activationSlot"]
                    .as_u64()
                    .is_some_and(|activation| activation <= slot)
            })
            .map(|f| json!({"id":f["id"],"activationSlot":f["activationSlot"]}))
            .collect::<Vec<_>>();
        features.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let ids = features
            .iter()
            .map(|f| {
                f["id"]
                    .as_str()
                    .ok_or_else(|| invalid("feature id missing"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let programs_hash = |kind: &str, entries: &[Value]| -> Result<Digest, Error> {
            let enabled = entries
                .iter()
                .filter(|p| {
                    p["enableFeatureId"].is_null()
                        || p["enableFeatureId"]
                            .as_str()
                            .is_some_and(|id| ids.contains(&id))
                })
                .filter(|p| {
                    p.get("migrationFeatureId").is_none_or(|f| {
                        f.is_null() || !f.as_str().is_some_and(|id| ids.contains(&id))
                    })
                })
                .collect::<Vec<_>>();
            let body = serde_json::to_string(&enabled).map_err(|e| invalid(&e.to_string()))?;
            Ok(Digest::of(format!("{kind}/v1\n{body}\n")))
        };
        let mut binding = json!({"schema":"svm-simulate-m6-runtime-binding/v1",
            "targetSlot":slot,"runtimeProfileId":profile.profile_id,"executor":executor,
            "features":features,"activeFeatureSetHash":Digest::of(format!("svm-feature-set/v1\n{}\n",ids.join("\n"))),
            "enabledBuiltinsHash":programs_hash("builtins", &self.builtins)?,
            "enabledPrecompilesHash":programs_hash("precompiles", &self.precompiles)?});
        binding["bindingHash"] = json!(Digest::of(format!("m6-runtime-binding/v1\n{binding}\n")));
        svm_replay_protocol::runtime::validate_runtime_binding(&binding)?;
        Ok(binding)
    }
}
