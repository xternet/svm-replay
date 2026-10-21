use super::*;
use crate::arguments::help;

#[derive(Args, Default)]
pub(in super::super::super) struct Options {
    /// Transaction to replay; its history is downloaded once and cached.
    #[arg(long, value_name = "SIGNATURE", group = "replay_target", help_heading = help::REPLAY,
        long_help = "Transaction to replay; its history is downloaded once and cached.\n\n\
                     The replay of the original is checked against what happened on chain. Needs an\n\
                     Alchemy key: API_ALCHEMY, a .env file or --alchemy-key-file.")]
    pub tx: Option<String>,
    /// Execute after a transaction signature or at the next produced slot after a slot number.
    #[arg(long, value_name = "SIGNATURE|SLOT", group = "replay_target", help_heading = help::REPLAY,
        requires = "payload", conflicts_with_all = ["tx", "demo", "replace", "append"])]
    pub after: Option<String>,
    /// One transaction or an ordered array to execute at --after (JSON or @FILE).
    #[arg(long, value_name = "JSON|@FILE", help_heading = help::WHAT_IF, requires = "after")]
    pub(in super::super) payload: Option<String>,
    /// Run these transactions instead of it, on the same state (JSON or @FILE).
    #[arg(long, value_name = "JSON|@FILE", help_heading = help::WHAT_IF, requires = "tx",
        long_help = "Run these transactions instead of it, on the same historical state, as JSON or\n\
                     @FILE: {\"transactionBase64\": \"...\"}, or an array of them run in order (a bundle,\n\
                     at most 16). The original still runs first and is checked.")]
    pub(in super::super) replace: Option<String>,
    /// Run these transactions right after it (JSON or @FILE).
    #[arg(long, value_name = "JSON|@FILE", help_heading = help::WHAT_IF, requires = "tx",
        conflicts_with = "replace",
        long_help = "Run these transactions right after it, on the state it left, as JSON or @FILE:\n\
                     {\"transactionBase64\": \"...\"}, or an array of them run in order (at most 16).\n\
                     Each one gets its own result.")]
    pub(in super::super) append: Option<String>,
    /// Set any account's state first, like eth_call state overrides (JSON or @FILE).
    #[arg(long = "state-overrides", visible_alias = "overrides", value_name = "JSON|@FILE", help_heading = help::WHAT_IF, requires = "replay_target",
        long_help = "Set any account's state first, existing or new, like eth_call state overrides,\n\
                     as a JSON array or @FILE. Fields: pubkey, lamports, owner, executable, rentEpoch,\n\
                     dataBase64. With owner the account is replaced whole; without it, the given\n\
                     fields change the current one. \"lamports\": \"0\" deletes it (or declares a new\n\
                     account); \"program\": \"@arb.so\" puts a program there. Applied just before the\n\
                     first transaction that runs.")]
    pub(in super::super) overrides: Option<String>,
    /// Print only these result fields (comma-separated); errors and checks always show.
    #[arg(long, value_name = "LIST", help_heading = help::OUTPUT, requires = "replay_target", value_delimiter = ',')]
    pub(in super::super) fields: Vec<String>,
    /// Save the JSON as DIR/<time>-<signature>.json and print only a short summary.
    #[arg(long, value_name = "DIR", help_heading = help::OUTPUT, requires = "replay_target")]
    pub(in super::super) out: Option<PathBuf>,
    /// Read the Alchemy key from FILE instead of API_ALCHEMY.
    #[arg(long, value_name = "FILE", help_heading = help::PROVIDER, requires = "replay_target",
        conflicts_with_all = ["historical_key", "historical_key_file"])]
    pub(in super::super) alchemy_key_file: Option<PathBuf>,
    /// Download the history again; don't read or fill the cache.
    #[arg(long, help_heading = help::PROVIDER, requires = "replay_target")]
    pub(in super::super) no_cache: bool,
    /// Keep runs and the cache in DIR instead of the default data folder.
    #[arg(long, value_name = "DIR", help_heading = help::PROVIDER, requires = "replay_target")]
    pub(in super::super) data_dir: Option<PathBuf>,
    #[command(flatten)]
    pub(in super::super) historical: historical::Options,
    #[command(flatten)]
    pub(in super::super) controls: controls::Controls,
    #[command(flatten)]
    pub(in super::super) runtime: super::super::super::RuntimeArgs,
}
