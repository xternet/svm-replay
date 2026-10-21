use super::*;

pub(in super::super) fn run(
    request: InitializationRequest,
    payload: Option<WhatIf>,
    config: &Config,
    sources: CompositeSource,
    cancellation: CancellationToken,
    retain: bool,
) -> Result<(Value, Option<SessionHydration>), Error> {
    let budget = ExecutionBudget::new(
        Duration::from_millis(request.limits.timeout_ms),
        cancellation,
    )
    .map_err(svm_replay_engine::shared::runtime::protocol_error)?;
    if retain {
        if payload.is_some() {
            return Err(Error::new(
                "INVALID_SESSION_INPUT",
                "session initialization cannot execute a payload",
            ));
        }
        let hydration = SessionHydration::initialized(
            request.clone(),
            config.clone(),
            sources.fresh_journal(),
        )?;
        Ok((
            historical::initialize(request, config, sources, &budget)?,
            Some(hydration),
        ))
    } else {
        let payload = payload.ok_or_else(|| {
            Error::new(
                "INVALID_REQUEST",
                "--after slot requires --payload or session mode",
            )
        })?;
        Ok((
            historical::execute_initialized_payload(request, payload, config, sources, &budget)?,
            None,
        ))
    }
}
