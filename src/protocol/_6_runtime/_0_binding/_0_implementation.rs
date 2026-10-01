use super::*;

pub fn validate_runtime_binding(binding: &Value) -> Result<(), Error> {
    let invalid = |message: &str| Error::new("RUNTIME_BINDING", message);
    let object = binding
        .as_object()
        .ok_or_else(|| invalid("runtime binding must be an object"))?;
    let fields = [
        "schema",
        "targetSlot",
        "runtimeProfileId",
        "executor",
        "features",
        "activeFeatureSetHash",
        "enabledBuiltinsHash",
        "enabledPrecompilesHash",
        "bindingHash",
    ];
    if object.len() != fields.len()
        || fields.iter().any(|field| !object.contains_key(*field))
        || binding["schema"] != "svm-simulate-m6-runtime-binding/v1"
    {
        return Err(invalid("runtime binding shape/schema differs"));
    }
    let slot = binding["targetSlot"]
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| invalid("invalid runtime target slot"))?;
    if !binding["executor"].is_object() {
        return Err(invalid("executor identity is missing"));
    }
    for name in [
        "runtimeProfileId",
        "activeFeatureSetHash",
        "enabledBuiltinsHash",
        "enabledPrecompilesHash",
        "bindingHash",
    ] {
        Digest::new(
            binding[name]
                .as_str()
                .ok_or_else(|| invalid("runtime hash missing"))?,
        )?;
    }
    let features = binding["features"]
        .as_array()
        .ok_or_else(|| invalid("feature activations missing"))?;
    let mut ids = Vec::<&str>::new();
    for feature in features {
        let item = feature
            .as_object()
            .ok_or_else(|| invalid("feature must be an object"))?;
        let id = feature["id"]
            .as_str()
            .ok_or_else(|| invalid("feature pubkey missing"))?;
        let bytes = bs58::decode(id)
            .into_vec()
            .map_err(|_| invalid("feature pubkey invalid"))?;
        if item.len() != 2
            || bytes.len() != 32
            || bs58::encode(bytes).into_string() != id
            || feature["activationSlot"].as_u64().is_none_or(|n| n > slot)
            || ids.last().is_some_and(|previous| *previous >= id)
        {
            return Err(invalid(
                "features must be canonical, ordered, unique and historically active",
            ));
        }
        ids.push(id);
    }
    let features_hash = Digest::of(format!("svm-feature-set/v1\n{}\n", ids.join("\n")));
    if binding["activeFeatureSetHash"] != serde_json::json!(features_hash) {
        return Err(invalid("feature-set hash differs from activations"));
    }
    let mut basis = object.clone();
    basis.remove("bindingHash");
    let bytes =
        serde_json::to_string(&basis).map_err(|e| Error::new("RUNTIME_BINDING", e.to_string()))?;
    if binding["bindingHash"]
        != serde_json::json!(Digest::of(format!("m6-runtime-binding/v1\n{bytes}\n")))
    {
        return Err(invalid("runtime binding hash differs from its contents"));
    }
    Ok(())
}
