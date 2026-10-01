use super::*;

/// `run` must execute and verify the whole original-control/requested attempt.
/// No state, output or verification from a discarded attempt is reused.
pub fn settle(
    base: &Value,
    mut run: impl FnMut(&Value) -> Result<Value, Error>,
    mut hydrate: impl FnMut(&str, u64) -> Result<Value, Error>,
) -> Result<Settled, Error> {
    let slot = target_slot(base)?;
    let complete = base["runtime"]["bankContext"]["requiredRuntimeSysvars"]
        .as_array()
        .ok_or_else(|| invalid("runtime requirements missing"))?
        .iter()
        .any(|v| v["requirement"] == "complete-generic-sysvar-context-v1");
    if complete {
        // Complete supplied contexts need no discovery conversion, but still pass
        // the same runtime guards and original-control verification.
        context::available_generic_sysvars(base)?;
        return Ok(Settled {
            fixture: base.clone(),
            output: run(base)?,
            attempts: Vec::new(),
        });
    }
    let mut inputs = initial_inputs(base)?;
    let mut attempts = Vec::new();
    for _ in 0..=GENERIC_SYSVARS.len() {
        let fixture = tracked_fixture(base, &inputs)?;
        match run(&fixture) {
            Ok(output) => {
                return Ok(Settled {
                    fixture,
                    output,
                    attempts,
                })
            }
            Err(error) if error.code == "NEEDS_INPUT" => {
                let details = error
                    .details
                    .as_ref()
                    .ok_or_else(|| invalid("missing guarded discovery evidence"))?;
                let response = if details.get("schema").is_some() {
                    details
                } else {
                    details
                        .get("cause")
                        .ok_or_else(|| invalid("missing cache discovery cause"))?
                };
                let response = context::parse_sysvar_discovery_response(
                    response,
                    &context::available_generic_sysvars(&fixture)?,
                )?;
                if response["status"] != "NEEDS_INPUT" {
                    return Err(invalid("non-discovery response in missing-input error"));
                }
                let key = response["pubkey"]
                    .as_str()
                    .ok_or_else(|| invalid("missing discovered pubkey"))?;
                let account = hydrate(key, slot).map_err(|error| {
                    let code = error.code.clone();
                    let message = error.message.clone();
                    Error::new(code, message)
                        .with_details(json!({"discovery":response,"sourceError":error}))
                })?;
                add_input(&mut inputs, key, account, slot)?;
                attempts.push(json!({"status":"DISCARDED_NEEDS_INPUT","response":response,
                    "fixtureSha256":Digest::of(canonical_json(&fixture)),"partialResultPublished":false}));
            }
            Err(error) => return Err(error),
        }
    }
    Err(invalid("finite generic-sysvar retry bound exhausted"))
}
