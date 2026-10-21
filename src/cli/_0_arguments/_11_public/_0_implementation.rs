use super::*;

pub(in super::super) fn commands(root: clap::Command) -> clap::Command {
    let globals: Vec<_> = root
        .get_arguments()
        .filter(|arg| arg.is_global_set() || arg.get_id() == "demo")
        .cloned()
        .map(|arg| {
            if arg.get_id() == "demo" {
                arg.hide(true)
            } else {
                arg
            }
        })
        .collect();
    root.mut_subcommand("fork", |command| {
        command
            .mut_arg("after", |arg| arg.required(true))
            .mut_arg("request", |arg| arg.hide(true))
    })
    .subcommand(invocation("tx", "tx", &globals))
    .subcommand(invocation("payload", "payload", &globals))
}

fn invocation(name: &'static str, positional: &str, globals: &[clap::Arg]) -> clap::Command {
    let command = clap::Command::new(name);
    let mut command = signature::Options::augment_args(command)
        .args(globals.iter().cloned())
        .mut_arg(positional, |arg| {
            arg.long(None::<&str>).index(1).required(true)
        });
    for hidden in ["tx", "after", "payload", "replace", "append"] {
        if hidden != positional && !(name == "payload" && hidden == "after") {
            command = command.mut_arg(hidden, |arg| arg.hide(true));
        }
    }
    if name == "tx" {
        command.about("Replay and verify an existing on-chain transaction")
    } else {
        command
            .about("Execute a new transaction or ordered bundle at historical state")
            .mut_arg("payload", |arg| arg.value_name("PAYLOAD"))
            .mut_arg("after", |arg| arg.required(true))
    }
}

pub(in super::super) fn parse(mut matches: clap::ArgMatches) -> Result<Args, clap::Error> {
    if let Some(("fork", input)) = matches.subcommand() {
        if !input.contains_id("after") {
            return Err(help::command().error(
                clap::error::ErrorKind::MissingRequiredArgument,
                "fork requires --after <SIGNATURE|SLOT>; use session for legacy descriptors",
            ));
        }
    }
    if matches!(matches.subcommand_name(), Some("tx" | "payload")) {
        let (_, input) = matches.remove_subcommand().expect("canonical subcommand");
        let options = signature::Options::from_arg_matches(&input)?;
        let mut args = Args::from_arg_matches(&matches)?;
        args.signature = options;
        Ok(args)
    } else {
        Args::from_arg_matches(&matches)
    }
}

pub(in super::super) fn complete(shell: clap_complete::Shell) -> Result<Value, Error> {
    clap_complete::generate(
        shell,
        &mut help::command(),
        "svm-replay",
        &mut std::io::stdout(),
    );
    Ok(json!({"outcome":"COMPLETED","schema":"svm-replay-completions/v1"}))
}

pub(in super::super) fn inspect_runtime(runtime: RuntimeArgs) -> Result<Value, Error> {
    let (catalog, catalog_sha256) = runtime.resolve()?;
    let installed = svm_replay_engine::_1_resolve_runtime::load_catalog(&catalog, &catalog_sha256)?;
    let runtimes = svm_replay_engine::_1_resolve_runtime::registry::Registry::bundled()?
        .installed_runtimes(&installed.manifest.workers)?;
    Ok(
        json!({"outcome":"COMPLETED","schema":"svm-replay-capabilities/v1",
        "workers":installed.manifest.workers,"runtimes":runtimes,"platform":installed.manifest.platform,
        "captureWorkers":installed.manifest.capture_workers,"preparedReplay":true,
        "sourceReconstruction":true,"releaseQualified":false}),
    )
}
