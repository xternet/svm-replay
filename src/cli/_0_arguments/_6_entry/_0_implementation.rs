use super::*;

pub fn run() -> (
    Result<Value, Error>,
    Option<svm_replay_engine::shared::progress::ProgressJob>,
) {
    use svm_replay_engine::shared::progress::{Phase, ProgressJob};
    let mut command = help::command();
    let matches = command.get_matches_mut();
    let args = public::parse(matches).unwrap_or_else(|e| e.format(&mut command).exit());
    let progress = args.progress.then(|| {
        ProgressJob::stderr(
            if args.signature.tx.is_some() || args.signature.after.is_some() {
                Phase::LocatingTransaction
            } else {
                Phase::ResolvingDependencies
            },
        )
    });
    (run_args(args), progress)
}

fn run_args(mut args: Args) -> Result<Value, Error> {
    let debug = matches!(&args.command, Some(Command::Simulate { debug: true, .. }));
    crate::json_output::configure(args.pretty, args.json || debug);
    if args.signature.tx.is_some() || args.signature.after.is_some() {
        if args.command.is_some() {
            return Err(Error::new(
                "INVALID_REQUEST",
                "--tx/--after cannot be combined with a subcommand",
            ));
        }
        super::super::super::terminal::configure(args.human);
        let result = signature::run(args.signature);
        report_result(&result, args.bug_report);
        return result;
    }
    if args.demo {
        if args.command.is_some() {
            return Err(Error::new(
                "INVALID_REQUEST",
                "--demo cannot be combined with a subcommand",
            ));
        }
        args.command = Some(Command::Demo {
            example: Some("default".into()),
            bundle: None,
            bundle_sha256: None,
            data_dir: None,
        });
    }
    if args.command.is_none() {
        if !args.json && !args.pretty && super::super::super::terminal::interactive() {
            super::super::super::terminal::configure(true);
            match super::super::super::terminal::select(
                "SVM Replay",
                &[
                    "Try offline demo",
                    "Replay your transaction (signature)",
                    "Advanced: load prepared replay JSON",
                    "Check installation",
                    "Help",
                ],
            )? {
                Some(0) => {
                    args.command = Some(Command::Demo {
                        example: None,
                        bundle: None,
                        bundle_sha256: None,
                        data_dir: None,
                    })
                }
                Some(1) => {
                    let result = match signature::collect()? {
                        Some(options) => signature::run(options),
                        None => Ok(json!({"outcome":"CANCELLED"})),
                    };
                    report_result(&result, args.bug_report);
                    return result;
                }
                Some(2) => match wizard::run()? {
                    Some(command) => {
                        args.command = Some(command);
                        args.human = true;
                    }
                    None => return Ok(json!({"outcome":"CANCELLED"})),
                },
                Some(3) => {
                    args.command = Some(Command::Doctor {
                        runtime: RuntimeArgs::default(),
                    })
                }
                Some(_) => return show_help(None),
                None => return Ok(json!({"outcome":"CANCELLED"})),
            }
        } else {
            return show_help(None);
        }
    }
    let command = args
        .command
        .ok_or_else(|| Error::new("INTERNAL", "command missing"))?;
    if (args.human || args.pretty) && matches!(&command, Command::Simulate { debug: true, .. }) {
        return Err(Error::new(
            "INVALID_REQUEST",
            "--human and --pretty cannot be used with --debug JSONL",
        ));
    }
    let human = args.human
        || (!args.json
            && !args.pretty
            && super::super::super::terminal::interactive()
            && matches!(
                &command,
                Command::Demo { .. } | Command::Fork { .. } | Command::Session { .. }
            ));
    super::super::super::terminal::configure(human);
    let result = execute(command);
    report_result(&result, args.bug_report);
    result
}

pub(in super::super) fn report_result(result: &Result<Value, Error>, path: Option<PathBuf>) {
    if let Some(path) = path {
        let error_value;
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                error_value = json!({"outcome":"ERROR","error":error});
                &error_value
            }
        };
        match super::super::super::report::write_if_internal(&path, value) {
            Ok(true) => eprintln!(
                "Bug-report draft saved to {}. Review before submitting: {}",
                path.display(),
                crate::report::ISSUE_URL
            ),
            Ok(false) => {}
            Err(error) => eprintln!(
                "BUG_REPORT_IO: {}: {error}; original outcome unchanged",
                path.display()
            ),
        }
    }
}
