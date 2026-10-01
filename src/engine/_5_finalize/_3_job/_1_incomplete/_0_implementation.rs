use super::*;

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Category {
    HistoricalAccountMissing,
    SlotHashesMissing,
    SysvarMissing,
    BankContextMissing,
    HistoricalDataUnavailable,
    RuntimeUnsupported,
    CapabilityUnsupported,
    DependencyUnsupported,
    ResourceLimit,
    ProviderError,
    VerificationMismatch,
    Timeout,
    Cancelled,
    Unclassified,
}

fn input(error: &Error) -> Value {
    let mut value = json!({});
    if let Some(details) = &error.details {
        for (field, paths) in [
            (
                "pubkey",
                &[
                    "/pubkey",
                    "/discovery/pubkey",
                    "/query/pubkey",
                    "/sourceError/details/query/pubkey",
                    "/cause/pubkey",
                ][..],
            ),
            (
                "slot",
                &[
                    "/parentSlot",
                    "/slot",
                    "/query/slot",
                    "/sourceError/details/query/slot",
                ][..],
            ),
            ("required", &["/required"][..]),
        ] {
            for path in paths {
                if let Some(found) = details.pointer(path).filter(|v| !v.is_null()) {
                    value[field] = found.clone();
                    break;
                }
            }
        }
    }
    value
}

fn category(code: &str, input: &Value) -> Category {
    use Category::*;
    match code {
        "UNSUPPORTED_HISTORICAL_ACCOUNT" | "UNSUPPORTED_EXCLUDED_ACCOUNT" => {
            HistoricalAccountMissing
        }
        "UNSUPPORTED_HISTORICAL_EPOCH_STAKE"
        | "UNSUPPORTED_BANK_INPUT"
        | "UNSUPPORTED_REWARD_RECOVERY" => BankContextMissing,
        "UNSUPPORTED_RUNTIME"
        | "UNSUPPORTED_RUNTIME_PROFILE"
        | "UNSUPPORTED_PLATFORM"
        | "UNSUPPORTED_TRANSACTION_VERSION" => RuntimeUnsupported,
        "CAPABILITY_UNAVAILABLE"
        | "UNSUPPORTED_RUNTIME_CAPABILITY"
        | "UNSUPPORTED_WORKER_CAPABILITY" => CapabilityUnsupported,
        "UNSUPPORTED_ALT_LIFECYCLE"
        | "UNSUPPORTED_PROGRAM_LIFECYCLE"
        | "UNSUPPORTED_FAILED_DURABLE_NONCE_STATE" => DependencyUnsupported,
        "UNSUPPORTED_RESOURCE_LIMIT"
        | "SOURCE_RESOURCE_LIMIT"
        | "BUDGET_EXCEEDED"
        | "WORKER_INPUTLIMIT"
        | "WORKER_OUTPUTLIMIT"
        | "CAPTURE_LIMIT" => ResourceLimit,
        "SOURCE_RATE_LIMIT" | "SOURCE_RPC_ERROR" | "SOURCE_HTTP_ERROR" | "SOURCE_TRANSPORT" => {
            ProviderError
        }
        "WORKER_TIMEOUT" | "SOURCE_DEADLINE" => Timeout,
        "CANCELLED" | "WORKER_CANCELLED" => Cancelled,
        "MISMATCH" | "TRACE_PARITY_MISMATCH" | "DEBUG_PARITY_MISMATCH" => VerificationMismatch,
        "UNSUPPORTED_REQUIRED_ACCOUNT"
        | "UNSUPPORTED_EXACT_SYSVAR_UNAVAILABLE"
        | "UNSUPPORTED_HIDDEN_SYSVAR_INPUT"
        | "SOURCE_UNAVAILABLE"
        | "NEEDS_INPUT" => match input["pubkey"].as_str() {
            Some("SysvarS1otHashes111111111111111111111111111") => SlotHashesMissing,
            Some(key) if crate::shared::bank::sysvars::GENERIC_SYSVARS.contains(&key) => {
                SysvarMissing
            }
            _ if matches!(
                code,
                "UNSUPPORTED_EXACT_SYSVAR_UNAVAILABLE" | "UNSUPPORTED_HIDDEN_SYSVAR_INPUT"
            ) =>
            {
                SysvarMissing
            }
            _ => HistoricalDataUnavailable,
        },
        _ => Unclassified,
    }
}

pub(in super::super) fn describe(error: &Error, receipt: &Value) -> Value {
    let input = input(error);
    let mut available = json!({});
    for name in [
        "sourceObservations",
        "sourcePreparation",
        "failedAttempts",
        "trace",
    ] {
        if let Some(value) = receipt.get(name).filter(|v| !v.is_null()) {
            available[name] = value.clone();
        }
    }
    if let Some(value) = error
        .details
        .as_ref()
        .and_then(|d| d.get("unverifiedOutput"))
    {
        available["unverifiedOutput"] = value.clone();
    }
    json!({"schema":"svm-replay-incomplete-evidence/v1",
        "category":category(&error.code, &input), "input":input,
        "complete":false, "estimated":false, "requestedReplayVerified":false,
        "originalControlVerified":receipt["controlVerification"]["status"] == "PASS",
        "available":available,
        "meaning":"Partial diagnostic evidence, not a verified requested replay or estimated final state. Artifact paths are relative to the receipt directory; verify their hashes before reading. Only the first blocker is reported; fixing it may reveal another."})
}
