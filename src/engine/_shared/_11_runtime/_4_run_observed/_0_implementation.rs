use super::*;

impl WorkerTransport {
    pub(in super::super) fn run_observed(
        &self,
        worker: &WorkerSpec,
        fixture: &Value,
        flags: &[String],
        output_files: &[PathBuf],
        limits: &WorkerLimits,
        budget: &ExecutionBudget,
        diagnostics: Option<&std::sync::mpsc::SyncSender<Vec<u8>>>,
    ) -> Result<WorkerOutput, WorkerError> {
        let started = Instant::now();
        budget.check()?;
        if output_files.iter().any(|path| !path.is_absolute()) {
            return Err(WorkerError::new(
                WorkerErrorCode::InvalidConfig,
                "auxiliary output paths must be absolute",
            ));
        }
        if limits.max_input_bytes == 0
            || limits.max_output_bytes == 0
            || limits.max_diagnostic_bytes == 0
            || limits.cleanup_grace.is_zero()
            || Instant::now().checked_add(limits.cleanup_grace).is_none()
        {
            return Err(WorkerError::new(
                WorkerErrorCode::InvalidConfig,
                "all byte limits and cleanup grace must be positive",
            ));
        }
        if flags.iter().any(|flag| flag.contains('\0')) {
            return Err(WorkerError::new(
                WorkerErrorCode::InvalidConfig,
                "worker flag contains NUL",
            ));
        }
        verify_pin(&worker.executable, &worker.sha256, Some(budget))?;
        if let Some(owner) = &self.owner {
            verify_pin(&owner.executable, &owner.sha256, Some(budget))?;
        }
        // Never silently redirect a missing cold-storage scratch root to /tmp.
        let scratch = tempfile::Builder::new()
            .prefix("svm-worker-")
            .tempdir_in(&self.scratch_root)
            .map_err(|error| WorkerError::io("create worker scratch", error))?;
        let input = scratch.path().join("input.json");
        let output = scratch.path().join("output.json");
        let result = (|| {
            write_input(&input, fixture, limits.max_input_bytes, budget)?;
            budget.check()?;
            let execution = process::execute(
                worker,
                self.owner.as_ref(),
                scratch.path(),
                &input,
                &output,
                flags,
                output_files,
                limits,
                budget,
                diagnostics,
            );
            // Recheck immutable identities on every execution terminal path.
            let postcheck = verify_pin(&worker.executable, &worker.sha256, None).and_then(|()| {
                if let Some(owner) = &self.owner {
                    verify_pin(&owner.executable, &owner.sha256, None)
                } else {
                    Ok(())
                }
            });
            let mut execution = match (execution, postcheck) {
                (Err(primary), Err(integrity)) => return Err(integrity.diagnostics_from(primary)),
                (Ok(observed), Err(mut integrity)) => {
                    integrity.pid = Some(observed.pid);
                    integrity.stdout = observed.stdout;
                    integrity.stderr = observed.stderr;
                    return Err(integrity);
                }
                (Err(primary), Ok(())) => return Err(primary),
                (Ok(observed), Ok(())) => observed,
            };
            let parsed: Result<Value, WorkerError> = (|| {
                budget.check()?;
                let bytes = read_bounded_file(&output, limits.max_output_bytes)?;
                budget.check()?;
                let value = svm_replay_protocol::parse_json(&bytes).map_err(|error| {
                    WorkerError::new(
                        WorkerErrorCode::InvalidOutput,
                        format!("worker output JSON: {error}"),
                    )
                })?;
                budget.check()?;
                Ok(value)
            })();
            let value = match parsed {
                Ok(value) => value,
                Err(mut error) => {
                    error.pid = Some(execution.pid);
                    error.stdout = execution.stdout;
                    error.stderr = execution.stderr;
                    return Err(error);
                }
            };
            Ok(WorkerOutput {
                value,
                pid: execution.pid,
                stdout: std::mem::take(&mut execution.stdout),
                stderr: std::mem::take(&mut execution.stderr),
                elapsed: started.elapsed(),
                cleanup_scope: if cfg!(windows) {
                    CleanupScope::WindowsJobObject
                } else if self.owner.is_some() && cfg!(target_os = "macos") {
                    CleanupScope::OwnerTerminatedProcessGroup
                } else if self.owner.is_some() {
                    CleanupScope::OwnerReapedProcessTree
                } else {
                    CleanupScope::ImmediateWorkerOnly
                },
            })
        })();
        match (result, scratch.close()) {
            (result, Ok(())) => result,
            (Err(primary), Err(error)) => Err(WorkerError::new(
                WorkerErrorCode::Cleanup,
                format!("remove owned scratch: {error}"),
            )
            .diagnostics_from(primary)),
            (Ok(observed), Err(error)) => {
                let mut error = WorkerError::new(
                    WorkerErrorCode::Cleanup,
                    format!("remove owned scratch: {error}"),
                );
                error.pid = Some(observed.pid);
                error.stdout = observed.stdout;
                error.stderr = observed.stderr;
                Err(error)
            }
        }
    }
}
