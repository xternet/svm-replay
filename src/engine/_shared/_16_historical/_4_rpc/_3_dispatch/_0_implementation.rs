use super::*;

fn count(request: &Request, min: usize, max: usize) -> Result<(), RpcError> {
    if (min..=max).contains(&request.params.len()) {
        Ok(())
    } else {
        Err(RpcError::params("wrong number of RPC arguments"))
    }
}

/// `svmReplay_advance` options `{unixTimestamp, syntheticEpoch}`: the time to set, and the
/// explicit opt-in to a synthetic test warp across epochs.
pub(in super::super) fn advance_options(
    options: Option<&Value>,
) -> Result<(Option<i64>, bool), RpcError> {
    let Some(options) = options.filter(|v| !v.is_null()) else {
        return Ok((None, false));
    };
    let options = options
        .as_object()
        .filter(|o| {
            o.keys()
                .all(|key| key == "unixTimestamp" || key == "syntheticEpoch")
        })
        .ok_or_else(|| RpcError::params("advance options: {unixTimestamp, syntheticEpoch}"))?;
    let unix_timestamp = match options.get("unixTimestamp").filter(|v| !v.is_null()) {
        None => None,
        Some(time) => Some(
            time.as_i64()
                .ok_or_else(|| RpcError::params("unixTimestamp must be an integer"))?,
        ),
    };
    let synthetic_epoch = match options.get("syntheticEpoch").filter(|v| !v.is_null()) {
        None => false,
        Some(flag) => flag
            .as_bool()
            .ok_or_else(|| RpcError::params("syntheticEpoch must be a boolean"))?,
    };
    Ok((unix_timestamp, synthetic_epoch))
}

pub(in super::super) fn dispatch(
    session: &mut VerifiedSession,
    request: &Request,
    budget: &ExecutionBudget,
) -> Result<(Value, bool), RpcError> {
    budget.check().map_err(protocol_error)?;
    let params = &request.params;
    match request.method.as_str() {
        "getHealth" => {
            count(request, 0, 0)?;
            session.request(SessionCommand::State, budget)?;
            Ok((json!("ok"), false))
        }
        "getGenesisHash" => {
            count(request, 0, 0)?;
            Ok((session.pins()["genesisHash"].clone(), false))
        }
        "getVersion" => {
            count(request, 0, 0)?;
            Ok((
                json!({"solana-core":session.pins()["runtime"]["agave"]}),
                false,
            ))
        }
        "getSlot" | "getBlockHeight" => {
            count(request, 0, 1)?;
            let state = session.request(SessionCommand::State, budget)?;
            let slot = state["slot"].as_u64().ok_or_else(|| {
                RpcError::from(Error::new("SESSION_PROTOCOL", "native slot missing"))
            })?;
            _2_accounts::config(params.first(), slot, false)?;
            Ok((
                if request.method == "getSlot" {
                    state["slot"].clone()
                } else if state["blockHeight"].is_u64() {
                    state["blockHeight"].clone()
                } else {
                    // Workers without slot advance report no height: the boundary's.
                    session.pins()["blockHeight"].clone()
                },
                false,
            ))
        }
        "getAccountInfo" | "getMultipleAccounts" | "getBalance" => {
            count(request, 1, 2)?;
            let state = session.request(SessionCommand::State, budget)?;
            let slot = state["slot"].as_u64().ok_or_else(|| {
                RpcError::from(Error::new("SESSION_PROTOCOL", "native slot missing"))
            })?;
            let mut config =
                _2_accounts::config(params.get(1), slot, request.method != "getBalance")?;
            // Balance needs no encoded bytes, and must work for large accounts.
            if request.method == "getBalance" {
                config["encoding"] = json!("base64");
                config["dataSlice"] = json!({"offset":0,"length":0});
            }
            let keys = if request.method == "getMultipleAccounts" {
                params[0]
                    .as_array()
                    .ok_or_else(|| RpcError::params("addresses must be an array"))?
                    .clone()
            } else {
                vec![params[0].clone()]
            };
            let values = _2_accounts::accounts(session, &keys, &config, budget)?;
            let value = if request.method == "getMultipleAccounts" {
                json!(values)
            } else if request.method == "getBalance" {
                if values[0].is_null() {
                    json!(0)
                } else {
                    values[0]["lamports"].clone()
                }
            } else {
                values[0].clone()
            };
            Ok((json!({"context":{"slot":slot},"value":value}), false))
        }
        "svmReplay_getPins" => {
            count(request, 0, 0)?;
            Ok((session.pins().clone(), false))
        }
        "svmReplay_getState" => {
            count(request, 0, 0)?;
            Ok((session.request(SessionCommand::State, budget)?, false))
        }
        "svmReplay_snapshot" | "svmReplay_reset" => {
            count(request, 0, 0)?;
            let command = if request.method == "svmReplay_snapshot" {
                SessionCommand::Snapshot
            } else {
                SessionCommand::Reset
            };
            Ok((session.request(command, budget)?, false))
        }
        "svmReplay_advance" => {
            count(request, 1, 2)?;
            let slots = params[0]
                .as_u64()
                .filter(|slots| *slots > 0)
                .ok_or_else(|| RpcError::params("slots must be a positive integer"))?;
            let (unix_timestamp, synthetic_epoch) = advance_options(params.get(1))?;
            Ok((
                session.request(
                    SessionCommand::Advance {
                        slots,
                        unix_timestamp,
                        synthetic_epoch,
                    },
                    budget,
                )?,
                false,
            ))
        }
        "svmReplay_selectFork" | "svmReplay_closeFork" => {
            count(request, 1, 1)?;
            let fork_id = params[0]
                .as_u64()
                .ok_or_else(|| RpcError::params("fork id must be an unsigned integer"))?;
            let command = if request.method == "svmReplay_selectFork" {
                SessionCommand::Select { fork_id }
            } else {
                SessionCommand::CloseFork { fork_id }
            };
            Ok((session.request(command, budget)?, false))
        }
        "svmReplay_setBalance" => {
            count(request, 2, 2)?;
            let pubkey = params[0]
                .as_str()
                .ok_or_else(|| RpcError::params("address must be a string"))?;
            crate::shared::history::address(pubkey, 32).map_err(RpcError::from)?;
            let lamports = params[1]
                .as_str()
                .ok_or_else(|| RpcError::params("lamports must be an unsigned decimal string"))?;
            if lamports.is_empty()
                || !lamports.bytes().all(|b| b.is_ascii_digit())
                || lamports.parse::<u64>().is_err()
            {
                return Err(RpcError::params("lamports must fit u64"));
            }
            Ok((
                session.request(
                    SessionCommand::SetBalance {
                        pubkey: pubkey.into(),
                        lamports: lamports.into(),
                    },
                    budget,
                )?,
                false,
            ))
        }
        "svmReplay_overrideAccounts" => {
            count(request, 1, 1)?;
            let entries: Vec<svm_replay_protocol::StateOverride> =
                serde_json::from_value(params[0].clone())
                    .map_err(|e| RpcError::params(format!("invalid account overrides: {e}")))?;
            if entries.is_empty() || entries.len() > svm_replay_protocol::MAX_STATE_OVERRIDES {
                return Err(RpcError::params("expected 1..=64 account overrides"));
            }
            for entry in &entries {
                entry.validate().map_err(RpcError::from)?;
            }
            Ok((
                session.request(SessionCommand::OverrideAccounts { entries }, budget)?,
                false,
            ))
        }
        "svmReplay_close" | "exit" => {
            count(request, 0, 0)?;
            Ok((json!({"closed":true}), true))
        }
        _ => _5_transactions::dispatch(session, request, budget),
    }
}
