use super::*;

pub(super) fn show_help(subcommand: Option<&str>) -> Result<Value, Error> {
    let mut command = Args::command();
    let command = match subcommand {
        Some(name) => command
            .find_subcommand_mut(name)
            .ok_or_else(|| Error::new("INTERNAL", "help command missing"))?,
        None => &mut command,
    };
    command
        .print_help()
        .map_err(|e| Error::new("TERMINAL_IO", e.to_string()))?;
    println!();
    Ok(json!({"outcome":"COMPLETED", "schema":"svm-replay-help/v1"}))
}
