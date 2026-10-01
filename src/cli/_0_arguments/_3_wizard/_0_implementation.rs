use super::*;

pub(super) fn input(prompt: &str, default: Option<&str>) -> Result<String, Error> {
    let mut input = Input::<String>::new().with_prompt(prompt);
    if let Some(value) = default {
        input = input.default(value.into());
    }
    input
        .interact_text()
        .map_err(|e| Error::new("TERMINAL_IO", e.to_string()))
}

pub(super) fn command(args: Vec<String>) -> Result<Command, Error> {
    Args::try_parse_from(args)
        .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?
        .command
        .ok_or_else(|| Error::new("INTERNAL", "wizard command missing"))
}

pub(in super::super) fn run() -> Result<Option<Command>, Error> {
    eprintln!("\nPrepared replay: use an existing request JSON, not a transaction signature.\nNo live provider fetching in this wizard.\nType /cancel to exit or /back to restart. Escape cancels menus.\n");
    'restart: loop {
        let mut values = Vec::new();
        for (prompt, default) in [
            ("1/5 Prepared request JSON path", None),
            ("2/5 Runtime manifest path (bundle.json or catalog)", None),
            ("3/5 Trusted manifest SHA-256 (from release metadata)", None),
            (
                "4/5 Data directory (default uses the platform data directory)",
                Some("default"),
            ),
        ] {
            let value = input(prompt, default)?;
            match value.as_str() {
                "/cancel" => return Ok(None),
                "/back" => continue 'restart,
                _ => values.push(value),
            }
        }
        let Some(kind) = super::super::super::terminal::select(
            "Runtime manifest type",
            &["Installed bundle", "Runtime catalog"],
        )?
        else {
            return Ok(None);
        };
        let Some(cache) = super::super::super::terminal::select(
            "5/5 Cache",
            &["Off", "Prepared boundaries", "All"],
        )?
        else {
            return Ok(None);
        };
        let validation = (|| {
            let bytes = read_bounded_file(std::path::Path::new(&values[0]), 256 * 1024 * 1024)
                .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
            PreparedRequest::parse(&bytes)?;
            Digest::new(values[2].clone())?;
            if !std::path::Path::new(&values[1]).is_file() {
                return Err(Error::new(
                    "INVALID_REQUEST",
                    "runtime manifest is not a file",
                ));
            }
            Ok::<(), Error>(())
        })();
        if let Err(error) = validation {
            eprintln!(
                "\nInput error: {}\nPlease correct the inputs.\n",
                crate::terminal::clean(&error.to_string())
            );
            continue;
        }
        let mut argv = vec![
            "svm-replay".into(),
            "simulate".into(),
            "--human".into(),
            "--request".into(),
            values[0].clone(),
        ];
        let (path_flag, pin_flag) = if kind == 0 {
            ("--bundle", "--bundle-sha256")
        } else {
            ("--catalog", "--catalog-sha256")
        };
        argv.extend([
            path_flag.into(),
            values[1].clone(),
            pin_flag.into(),
            values[2].clone(),
            "--cache".into(),
            ["off", "prepared", "all"][cache].into(),
        ]);
        if values[3] != "default" {
            argv.extend(["--data-dir".into(), values[3].clone()]);
        }
        let command = command(argv.clone())?;
        eprintln!(
            "\nEquivalent command ({}):\n  {}{}\n",
            if cfg!(windows) {
                "PowerShell"
            } else {
                "POSIX shell"
            },
            if cfg!(windows) { "& " } else { "" },
            argv.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
        );
        eprintln!(
            "Runtime identity and execution inputs will be checked by the existing replay engine."
        );
        match super::super::super::terminal::select(
            "Run this replay?",
            &["Run", "Back: edit inputs", "Cancel"],
        )? {
            Some(0) => return Ok(Some(command)),
            Some(1) => continue,
            _ => return Ok(None),
        }
    }
}

pub(super) fn quote(value: &str) -> String {
    let value = super::super::super::terminal::clean(value);
    if cfg!(windows) {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}
