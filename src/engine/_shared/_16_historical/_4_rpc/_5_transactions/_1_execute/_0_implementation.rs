use super::*;

fn result(output: &Value) -> Result<Value, RpcError> {
    let error = match &output["normalizedError"] {
        Value::Null => Value::Null,
        Value::String(error) => serde_json::from_str(error)
            .map_err(|_| Error::new("SESSION_PROTOCOL", "invalid native transaction error"))?,
        _ => return Err(Error::new("SESSION_PROTOCOL", "native transaction error missing").into()),
    };
    let units = output["computeUnits"]
        .as_u64()
        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "native compute units missing"))?;
    let logs = output["logs"]
        .as_array()
        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "native transaction logs missing"))?;
    let return_data = match &output["returnData"] {
        Value::Null => Value::Null,
        data if data.is_object() => {
            json!({"programId":data["programId"],"data":[data["dataBase64"],"base64"]})
        }
        _ => return Err(Error::new("SESSION_PROTOCOL", "native return data missing").into()),
    };
    Ok(
        json!({"err":error,"logs":logs,"unitsConsumed":units,"returnData":return_data,"accounts":null,"innerInstructions":null}),
    )
}

pub(in super::super) fn dispatch(
    session: &mut VerifiedSession,
    request: &Request,
    budget: &ExecutionBudget,
) -> Result<(Value, bool), RpcError> {
    count(request, 1, 2)?;
    let slot = slot(session, budget)?;
    let simulate = request.method == "simulateTransaction";
    let config = _0_options::config(request.params.get(1), slot, simulate)?;
    let transaction_base64 = _0_options::wire(&request.params[0], &config)?;
    let command = if simulate {
        SessionCommand::RpcSimulate {
            transaction_base64,
            sig_verify: _0_options::flag(&config, "sigVerify", false)?,
            replace_recent_blockhash: _0_options::flag(&config, "replaceRecentBlockhash", false)?,
        }
    } else {
        SessionCommand::RpcSend {
            transaction_base64,
            skip_preflight: _0_options::flag(&config, "skipPreflight", false)?,
        }
    };
    let response = session.request(command, budget)?;
    let mut value = result(&response["output"])?;
    if simulate {
        if let Some(replacement) = response.get("replacementBlockhash") {
            value["replacementBlockhash"] = replacement.clone();
        }
        return Ok((json!({"context":{"slot":slot},"value":value}), false));
    }
    if response["admitted"] == true {
        let signature = response["signature"].as_str().ok_or_else(|| {
            Error::new("SESSION_PROTOCOL", "native transaction signature missing")
        })?;
        return Ok((json!(signature), false));
    }
    if response["preflightFailed"] == true {
        return Err(RpcError {
            code: -32002,
            message: format!("Transaction simulation failed: {}", value["err"]),
            data: Some(value),
        });
    }
    Err(RpcError {
        code: -32003,
        message: format!("Transaction rejected: {}", value["err"]),
        data: Some(value),
    })
}
