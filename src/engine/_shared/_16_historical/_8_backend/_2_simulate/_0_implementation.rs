use super::*;

/// What the throwaway execution must reproduce before its account bytes stand for the
/// simulation: the same outcome, logs, units, return data, fee, account transitions and
/// runtime inner instructions.
const REPRODUCED: [&str; 8] = [
    "status",
    "normalizedError",
    "computeUnits",
    "logs",
    "returnData",
    "fee",
    "accountTransitions",
    "instructionTrace",
];

/// Simulates on `branch`; requested post-execution account bytes come from executing the
/// same transaction on a disposable copy of the branch, which is always discarded.
pub fn simulate(
    branches: &mut Branches,
    session: &mut impl Commands,
    branch: u64,
    request: Simulation,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    let simulation = branches.run(
        session,
        branch,
        SessionCommand::RpcSimulate {
            transaction_base64: request.transaction_base64.clone(),
            sig_verify: request.sig_verify,
            replace_recent_blockhash: request.replace_recent_blockhash,
        },
        budget,
    )?;
    if request.accounts.is_empty() || simulation["output"]["status"] != "ok" {
        return Ok(json!({"simulation": simulation, "postAccounts": null}));
    }
    let copy = branches.fork(session, branch, budget)?;
    let accounts = post_accounts(
        branches,
        session,
        copy,
        &request,
        &simulation["output"],
        budget,
    );
    let accounts = match (accounts, branches.close(session, copy, budget)) {
        (Ok(accounts), Ok(())) => accounts,
        (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
        (Err(error), Err(cleanup)) => return Err(with_cleanup(error, cleanup)),
    };
    Ok(json!({"simulation": simulation, "postAccounts": accounts}))
}

fn post_accounts(
    branches: &mut Branches,
    session: &mut impl Commands,
    copy: u64,
    request: &Simulation,
    simulated: &Value,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    // The raw command executes the message with the Bank's blockhash, as the
    // simulation did after admission; it differs only in committing the result.
    let executed = branches.run(
        session,
        copy,
        SessionCommand::Send {
            transaction_base64: request.transaction_base64.clone(),
            sig_verify: false,
        },
        budget,
    )?;
    for field in REPRODUCED {
        if executed[field] != simulated[field] {
            return Err(Error::new(
                "SIMULATION_DIVERGED",
                format!("the post-state execution did not reproduce the simulation's {field}"),
            )
            .with_details(
                json!({"field":field,"simulated":simulated[field],"executed":executed[field]}),
            ));
        }
    }
    let mut accounts = Vec::with_capacity(request.accounts.len());
    for chunk in request.accounts.chunks(100) {
        let reply = branches.run(
            session,
            copy,
            SessionCommand::Accounts {
                pubkeys: chunk.to_vec(),
            },
            budget,
        )?;
        let images = reply
            .as_array()
            .filter(|images| images.len() == chunk.len())
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "account images missing"))?;
        accounts.extend(images.iter().cloned());
    }
    Ok(Value::Array(accounts))
}

/// Keeps a failed cleanup visible beside the failure that caused it.
pub fn with_cleanup(mut error: Error, cleanup: Error) -> Error {
    let cleanup = json!(cleanup);
    match &mut error.details {
        Some(Value::Object(details)) => {
            details.insert("cleanupError".into(), cleanup);
        }
        details => {
            let previous = details.take();
            *details = Some(json!({"cleanupError": cleanup, "details": previous}));
        }
    }
    error
}
