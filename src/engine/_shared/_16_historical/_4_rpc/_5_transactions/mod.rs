use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
mod _0_options;
mod _1_execute;
mod _2_queries;
pub(super) fn dispatch(
    session: &mut VerifiedSession,
    request: &Request,
    budget: &ExecutionBudget,
) -> Result<(Value, bool), RpcError> {
    if ["sendTransaction", "simulateTransaction"].contains(&request.method.as_str()) {
        _1_execute::dispatch(session, request, budget)
    } else {
        _2_queries::dispatch(session, request, budget)
    }
}
fn slot(session: &mut VerifiedSession, budget: &ExecutionBudget) -> Result<u64, RpcError> {
    session.request(SessionCommand::State, budget)?["slot"]
        .as_u64()
        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "native slot missing").into())
}
fn count(request: &Request, min: usize, max: usize) -> Result<(), RpcError> {
    if (min..=max).contains(&request.params.len()) {
        Ok(())
    } else {
        Err(RpcError::params("wrong number of RPC arguments"))
    }
}
#[cfg(test)]
mod tests;
