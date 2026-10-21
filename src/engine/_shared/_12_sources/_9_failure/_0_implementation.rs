use super::*;

fn category(code: &str) -> Option<&'static str> {
    match code {
        "SOURCE_UNAVAILABLE" | "UNSUPPORTED_HISTORICAL_ACCOUNT" => {
            Some("PROVIDER_DATA_UNAVAILABLE")
        }
        "SOURCE_RATE_LIMIT" | "SOURCE_RPC_ERROR" | "SOURCE_HTTP_ERROR" | "SOURCE_TRANSPORT"
        | "SOURCE_DEADLINE" => Some("PROVIDER_REQUEST_FAILED"),
        _ => None,
    }
}

fn attribute(mut error: Error, identities: impl Iterator<Item = Value>) -> Error {
    if error
        .details
        .as_ref()
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        == Some("unproven-account-absence")
    {
        return error;
    }
    if error
        .details
        .as_ref()
        .and_then(|value| value.get("providerFailure"))
        .is_some()
    {
        return error;
    }
    let kind = category(&error.code).or_else(|| {
        error
            .details
            .as_ref()?
            .pointer("/sourceError/code")?
            .as_str()
            .and_then(category)
    });
    let Some(kind) = kind else {
        return error;
    };
    let mut providers = BTreeMap::new();
    for identity in identities {
        let (Some(id), Some(source_kind)) = (identity["id"].as_str(), identity["kind"].as_str())
        else {
            continue;
        };
        if matches!(source_kind, "captured-history" | "supplied-bank") {
            continue;
        }
        let name = if source_kind == "alchemy-historical" {
            "Alchemy"
        } else {
            id
        };
        providers.insert(
            id.to_owned(),
            json!({"id":id,"name":name,"kind":source_kind}),
        );
    }
    if providers.is_empty() {
        return error;
    }
    let names = providers
        .values()
        .filter_map(|value| value["name"].as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let label = if kind == "PROVIDER_DATA_UNAVAILABLE" {
        "Missing historical data from provider"
    } else {
        "Provider request failed"
    };
    error.message = format!("{label}: {names}. {}", error.message);
    let mut details = match error.details.take() {
        None => json!({}),
        Some(value) if value.is_object() => value,
        Some(value) => json!({"originalDetails":value}),
    };
    details["providerFailure"] =
        json!({"kind":kind,"providers":providers.into_values().collect::<Vec<_>>()});
    error.details = Some(details);
    error
}

pub(crate) fn attribute_failure(error: Error, sources: &CompositeSource) -> Error {
    let mut identities = Vec::new();
    if let Some(details) = &error.details {
        let pubkey = details
            .get("pubkey")
            .or_else(|| details.pointer("/discovery/pubkey"))
            .and_then(Value::as_str);
        let slot = details
            .get("parentSlot")
            .or_else(|| details.get("slot"))
            .and_then(Value::as_u64);
        if let Some(pubkey) = pubkey {
            identities.extend(
                sources
                    .recorded
                    .iter()
                    .filter(|observation| {
                        observation["query"]["pubkey"] == pubkey
                            && slot.is_none_or(|slot| observation["query"]["slot"] == slot)
                    })
                    .map(|observation| observation["source"].clone()),
            );
        }
        for pointer in ["/observations", "/sourceError/details/observations"] {
            if let Some(observations) = details.pointer(pointer).and_then(Value::as_array) {
                identities.extend(
                    observations
                        .iter()
                        .map(|observation| observation["source"].clone()),
                );
            }
        }
        if let Some(providers) = details
            .pointer("/sourceError/details/providerFailure/providers")
            .and_then(Value::as_array)
        {
            identities.extend(providers.iter().cloned());
        }
    }
    attribute(error, identities.into_iter())
}

pub(crate) fn attribute_source_failure(error: SourceError, identity: &Value) -> SourceError {
    let code = error.code;
    let attributed = attribute(
        Error {
            code: code.into(),
            message: error.message,
            details: error.details,
        },
        std::iter::once(identity.clone()),
    );
    SourceError {
        code,
        message: attributed.message,
        details: attributed.details,
    }
}
