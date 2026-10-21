use super::*;

/// A pinned runtime as users name it: the official Agave release, with the LiteSVM executor
/// identified separately. `family` stays the stable machine ID (`v2-2` … `v4-2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeName {
    pub family: String,
    pub agave: String,
    pub litesvm: String,
    /// Slots of its reviewed profiles: experimental qualification, not proof of exact support.
    pub reviewed_slots: Vec<(u64, u64)>,
}

fn invalid(message: String) -> Error {
    Error::new("RUNTIME_REGISTRY", message)
}

fn text<'a>(executor: &'a Value, key: &str) -> Result<&'a str, Error> {
    executor[key]
        .as_str()
        .ok_or_else(|| invalid(format!("executor {key} missing")))
}

impl RuntimeName {
    /// The name shown to users, e.g. `Agave 2.3.9`.
    pub fn name(&self) -> String {
        format!("Agave {}", self.agave)
    }
    pub fn to_json(&self) -> Value {
        json!({"family":self.family,"name":self.name(),"agave":self.agave,"litesvm":self.litesvm,
            "reviewedSlots":self.reviewed_slots.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>()})
    }
}

impl Registry {
    /// The official name of a pinned executor, derived from its provenance: the program-runtime and
    /// feature-set pins must agree with each other and with the executor ID. Unknown or inconsistent
    /// executors are errors, never guesses.
    pub fn runtime_name(&self, executor_source_id: &str) -> Result<RuntimeName, Error> {
        let executor = self
            .executors
            .iter()
            .find(|e| e["id"] == executor_source_id)
            .ok_or_else(|| {
                Error::new(
                    "UNSUPPORTED_RUNTIME",
                    format!("{executor_source_id} is not a reviewed runtime executor"),
                )
            })?;
        let agave = text(executor, "solanaProgramRuntimeVersion")?;
        let litesvm = text(executor, "litesvmVersion")?;
        let expected = format!("litesvm-{}-agave-{agave}", litesvm.replace('+', "-"));
        if text(executor, "agaveFeatureSetVersion")? != agave || executor_source_id != expected {
            return Err(invalid(format!(
                "{executor_source_id}: runtime pins disagree with the executor ID"
            )));
        }
        let mut slots: Vec<(u64, u64)> = (self.profiles.iter())
            .filter(|p| p.executor_source_id == executor_source_id)
            .map(|p| (p.earliest_slot, p.latest_slot))
            .collect();
        slots.sort_unstable();
        let mut reviewed_slots: Vec<(u64, u64)> = Vec::new();
        for (first, last) in slots {
            match reviewed_slots.last_mut() {
                Some(previous) if first <= previous.1.saturating_add(1) => {
                    previous.1 = previous.1.max(last)
                }
                _ => reviewed_slots.push((first, last)),
            }
        }
        Ok(RuntimeName {
            family: text(executor, "packageName")?.to_owned(),
            agave: agave.to_owned(),
            litesvm: litesvm.trim_start_matches('v').to_owned(),
            reviewed_slots,
        })
    }
    /// The pinned runtime of a family ID, for messages about requested families.
    pub fn runtime_name_of_family(&self, family: &str) -> Result<RuntimeName, Error> {
        let executor = (self.executors.iter())
            .find(|e| e["packageName"] == family)
            .ok_or_else(|| {
                Error::new(
                    "UNSUPPORTED_RUNTIME",
                    format!("runtime family {family} is not reviewed"),
                )
            })?;
        self.runtime_name(text(executor, "id")?)
    }
    /// The official runtime of a worker, or `None` for an explicit executor that is not pinned in
    /// this registry (advanced runtime inputs). A pinned executor must match the worker's family.
    pub fn worker_runtime(&self, worker: &WorkerDescriptor) -> Result<Option<RuntimeName>, Error> {
        let name = match self.runtime_name(&worker.executor_source_id) {
            Err(error) if error.code == "UNSUPPORTED_RUNTIME" => return Ok(None),
            other => other?,
        };
        if name.family != worker.family {
            return Err(invalid(format!(
                "{}: worker family {} differs from {}",
                worker.executor_source_id, worker.family, name.family
            )));
        }
        Ok(Some(name))
    }
    /// A worker's runtime for receipts: its official name, or `null` when it is not pinned.
    pub fn worker_runtime_json(&self, worker: &WorkerDescriptor) -> Result<Value, Error> {
        Ok(self
            .worker_runtime(worker)?
            .map_or(Value::Null, |name| name.to_json()))
    }
    /// The official runtime of each installed worker, as `capabilities` reports it.
    pub fn installed_runtimes(&self, workers: &[WorkerDescriptor]) -> Result<Vec<Value>, Error> {
        (workers.iter())
            .map(|worker| self.worker_runtime_json(worker))
            .collect()
    }
    /// Every pinned runtime, oldest first.
    pub fn runtime_names(&self) -> Result<Vec<RuntimeName>, Error> {
        let mut names = (self.executors.iter())
            .map(|e| self.runtime_name(text(e, "id")?))
            .collect::<Result<Vec<_>, _>>()?;
        names.sort_by_key(|n| n.reviewed_slots.first().copied());
        Ok(names)
    }
}
