use super::*;

pub(super) fn input(prompt: &str, optional: bool) -> Result<Option<String>, Error> {
    let value: String = Input::new()
        .with_prompt(prompt)
        .allow_empty(optional)
        .interact_text()
        .map_err(|e| Error::new("TERMINAL_IO", e.to_string()))?;
    if value == "/cancel" {
        return Ok(None);
    }
    Ok(Some(value))
}
pub(in crate::_0_arguments) fn collect() -> Result<Option<Options>, Error> {
    eprintln!("\nReplay a historical transaction. No wallet or broadcast.\nAlchemy requests may consume your plan credits. Type /cancel to leave.\n");
    let Some(tx) = input("Transaction signature", false)? else {
        return Ok(None);
    };
    svm_replay_engine::shared::history::address(&tx, 64)?;
    let mut options = Options {
        tx: Some(tx),
        ..Options::default()
    };
    super::super::credentials::load_dotenv()?;
    if std::env::var("API_ALCHEMY").is_err() {
        let Some(path) = input("Alchemy key file path (key stays local)", false)? else {
            return Ok(None);
        };
        options.alchemy_key_file = Some(PathBuf::from(path));
    }
    for (prompt, target) in [
        (
            "Replacement JSON or @file (Enter: original)",
            &mut options.replace,
        ),
        (
            "Override JSON or @file (Enter: none)",
            &mut options.overrides,
        ),
    ] {
        let Some(value) = input(prompt, true)? else {
            return Ok(None);
        };
        if !value.is_empty() {
            *target = Some(value);
        }
    }
    let Some(collection) = input(
        "Collect all, JSON or @file (Enter: standard result; example {\"calls\":true})",
        true,
    )?
    else {
        return Ok(None);
    };
    if !collection.is_empty() {
        super::super::inputs::collect(&collection)?;
        options.controls.collect = Some(collection);
    }
    let Some(out) = input("Output directory (Enter: terminal)", true)? else {
        return Ok(None);
    };
    if !out.is_empty() {
        options.out = Some(PathBuf::from(out));
    }
    let Some(settings) = crate::terminal::select(
        "Settings",
        &["Use defaults", "Customize fields, cache and limits"],
    )?
    else {
        return Ok(None);
    };
    if settings == 1 {
        let Some(fields) = input("Fields, comma-separated (Enter: all)", true)? else {
            return Ok(None);
        };
        if !fields.is_empty() {
            options.fields = fields.split(',').map(|s| s.trim().to_owned()).collect();
            super::super::output::validate_fields(&options.fields)?;
        }
        let Some(cache) =
            crate::terminal::select("Cache", &["Reuse historical inputs", "No cache"])?
        else {
            return Ok(None);
        };
        options.no_cache = cache == 1;
        let Some(dir) = input("Data directory (Enter: OS default)", true)? else {
            return Ok(None);
        };
        if !dir.is_empty() {
            options.data_dir = Some(PathBuf::from(dir));
        }
        let Some(timeout) = input("Timeout seconds (Enter: 900)", true)? else {
            return Ok(None);
        };
        if !timeout.is_empty() {
            options.controls.timeout = Some(
                timeout
                    .parse()
                    .map_err(|_| Error::new("INVALID_REQUEST", "timeout must be an integer"))?,
            );
        }
        let Some(requests) = input("Maximum RPC requests (Enter: 5000)", true)? else {
            return Ok(None);
        };
        if !requests.is_empty() {
            options.controls.max_requests =
                Some(requests.parse().map_err(|_| {
                    Error::new("INVALID_REQUEST", "request budget must be an integer")
                })?);
        }
        options.controls.limits()?;
    }
    match crate::terminal::select("Fetch historical data and replay?", &["Run", "Cancel"])? {
        Some(0) => Ok(Some(options)),
        _ => Ok(None),
    }
}
