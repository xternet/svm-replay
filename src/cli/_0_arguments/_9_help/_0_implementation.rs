use super::*;
use clap::builder::{
    styling::{AnsiColor, Effects, Style, Styles},
    StyledStr,
};

const DIM: Style = Style::new().effects(Effects::DIMMED);
const HEADER: Style = AnsiColor::Green.on_default().effects(Effects::BOLD);
const LITERAL: Style = AnsiColor::Cyan.on_default().effects(Effects::BOLD);
const PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
/// Option groups. The help shows them in the order their first option is declared.
pub(in super::super) const REPLAY: &str = "Replay options";
pub(in super::super) const WHAT_IF: &str = "What-if options";
pub(in super::super) const OUTPUT: &str = "Output options";
pub(in super::super) const PROVIDER: &str = "Provider and cache options";
pub(in super::super) const LIMITS: &str = "Limit options";
pub(in super::super) const PINNED: &str = "Pinned input options (advanced)";
pub(in super::super) fn command() -> clap::Command {
    let styles = Styles::styled()
        .header(HEADER)
        .usage(HEADER)
        .literal(LITERAL)
        .placeholder(PLACEHOLDER)
        .context(DIM)
        .context_value(DIM);
    let name = format!("{LITERAL}svm-replay{LITERAL:#}");
    let command = public::commands(Args::command())
        .styles(styles)
        .override_usage(format!(
            "{name} {LITERAL}tx{LITERAL:#} {PLACEHOLDER}<SIGNATURE>{PLACEHOLDER:#} [OPTIONS]\n       \
             {name} {LITERAL}payload{LITERAL:#} <PAYLOAD> --after <SIGNATURE|SLOT> [OPTIONS]\n       \
             {name} {LITERAL}fork{LITERAL:#} --after <SIGNATURE|SLOT> [OPTIONS]\n       \
             {name} {PLACEHOLDER}<COMMAND>{PLACEHOLDER:#} [OPTIONS]"
        ))
        .after_long_help(format!(
            "{HEADER}Examples:{HEADER:#}\n  svm-replay demo\n  svm-replay tx <SIGNATURE> --human\n  \
             svm-replay tx <SIGNATURE> --state-overrides @state.json\n  \
             svm-replay payload @bundle.json --after <SIGNATURE>\n  \
             svm-replay fork --after <SLOT>\n\n{DIM}The Alchemy key comes from API_ALCHEMY or a .env file in \
             the current folder.\nMore: https://github.com/xternet/svm-replay{DIM:#}"
        ));
    let mut command = spaced(command);
    command.build();
    shade(command)
}

/// `-h` and `--help` print the same full page: every option on its own line, its description below
/// and a blank line before the next one (clap spaces options when a command has long text).
fn spaced(command: clap::Command) -> clap::Command {
    let command = match (command.get_about().cloned(), command.get_long_about()) {
        (Some(about), None) => command.long_about(about),
        _ => command,
    };
    // The fork stub hands every argument, `--help` included, to the npm command.
    let command = if command.is_disable_help_flag_set() {
        command
    } else {
        command.disable_help_flag(true).arg(
            clap::Arg::new("help")
                .short('h')
                .long("help")
                .action(clap::ArgAction::HelpLong)
                .help("Print help"),
        )
    };
    command.mut_subcommands(spaced)
}

/// Descriptions in a dimmer color than the commands and options they describe.
fn shade(command: clap::Command) -> clap::Command {
    command
        .mut_args(|mut arg| {
            if let Some(help) = arg.get_help().map(dim) {
                arg = arg.help(help);
            }
            match arg.get_long_help().map(dim) {
                Some(long) => arg.long_help(long),
                None => arg,
            }
        })
        .mut_subcommands(|subcommand| match subcommand.get_about().map(dim) {
            Some(about) => shade(subcommand.about(about)),
            None => shade(subcommand),
        })
}

fn dim(text: &StyledStr) -> StyledStr {
    format!("{DIM}{text}{DIM:#}").into()
}
