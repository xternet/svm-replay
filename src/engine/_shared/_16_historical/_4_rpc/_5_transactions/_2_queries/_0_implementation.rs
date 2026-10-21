use super::*;

pub(in super::super) fn dispatch(
    session: &mut VerifiedSession,
    request: &Request,
    budget: &ExecutionBudget,
) -> Result<(Value, bool), RpcError> {
    let params = &request.params;
    match request.method.as_str() {
        "getLatestBlockhash" => {
            count(request, 0, 1)?;
            let slot = slot(session, budget)?;
            super::super::super::_2_accounts::config(params.first(), slot, false)?;
            let value = session.request(SessionCommand::LatestBlockhash, budget)?;
            Ok((json!({"context":{"slot":slot},"value":value}), false))
        }
        "getSignatureStatuses" => {
            count(request, 1, 2)?;
            let signatures = params[0]
                .as_array()
                .filter(|v| v.len() <= 256)
                .ok_or_else(|| RpcError::params("at most 256 signatures required"))?
                .iter()
                .map(|value| {
                    let signature = value
                        .as_str()
                        .ok_or_else(|| RpcError::params("signature must be a string"))?;
                    crate::shared::history::address(signature, 64)
                        .map_err(|_| RpcError::params("invalid signature"))?;
                    Ok(signature.to_owned())
                })
                .collect::<Result<Vec<_>, RpcError>>()?;
            if let Some(config) = params.get(1).filter(|v| !v.is_null()) {
                let object = config
                    .as_object()
                    .ok_or_else(|| RpcError::params("invalid status config"))?;
                if object.keys().any(|key| key != "searchTransactionHistory") {
                    return Err(RpcError::params("unsupported status config"));
                }
                _0_options::flag(config, "searchTransactionHistory", false)?;
            }
            let slot = slot(session, budget)?;
            let value =
                session.request(SessionCommand::SignatureStatuses { signatures }, budget)?;
            Ok((json!({"context":{"slot":slot},"value":value}), false))
        }
        "getMinimumBalanceForRentExemption" | "getFeeForMessage" => {
            count(request, 1, 2)?;
            let slot = slot(session, budget)?;
            super::super::super::_2_accounts::config(params.get(1), slot, false)?;
            let command = if request.method == "getFeeForMessage" {
                let message = params[0]
                    .as_str()
                    .ok_or_else(|| RpcError::params("base64 message required"))?;
                if message.len() > 1644 {
                    return Err(RpcError::params("message exceeds packet limit"));
                }
                SessionCommand::FeeForMessage {
                    message_base64: message.into(),
                }
            } else {
                let size = params[0]
                    .as_u64()
                    .filter(|size| *size <= 10 * 1024 * 1024)
                    .ok_or_else(|| RpcError::params("account data length must be 0..10MiB"))?;
                SessionCommand::Rent {
                    data_len: usize::try_from(size)
                        .map_err(|_| RpcError::params("account size overflow"))?,
                }
            };
            let value = session.request(command, budget)?;
            Ok((
                if request.method == "getFeeForMessage" {
                    json!({"context":{"slot":slot},"value":value})
                } else {
                    value
                },
                false,
            ))
        }
        _ => Err(RpcError::new(-32601, "Method not found")),
    }
}
