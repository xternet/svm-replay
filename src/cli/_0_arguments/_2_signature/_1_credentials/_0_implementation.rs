use super::*;

pub(super) fn invalid(message: &str) -> Error {
    Error::new("SOURCE_CONFIGURATION", message)
}
pub(super) fn validate(key: &str) -> Result<(), Error> {
    if key.is_empty()
        || key.len() > 512
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid(
            "API_ALCHEMY must be a nonempty API key, not an expression",
        ));
    }
    Ok(())
}

pub(in super::super) fn load_dotenv() -> Result<(), Error> {
    if let Some(key) = std::env::var_os("API_ALCHEMY") {
        return validate(
            key.to_str()
                .ok_or_else(|| invalid("API_ALCHEMY must be UTF-8"))?,
        );
    }
    match std::fs::symlink_metadata(".env") {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(invalid("cannot inspect .env")),
    }
    let bytes = read_bounded_file(Path::new(".env"), 64 * 1024)
        .map_err(|_| invalid("cannot read .env (maximum 64 KiB)"))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid(".env must be UTF-8"))?;
    if let Some(key) = parse(text)? {
        // CLI only, before worker or progress threads start.
        std::env::set_var("API_ALCHEMY", key);
    }
    Ok(())
}

pub(super) fn parse(text: &str) -> Result<Option<&str>, Error> {
    let mut found = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        let line = if let Some(rest) = line.strip_prefix("export") {
            if rest.starts_with(char::is_whitespace) {
                rest.trim_start()
            } else {
                line
            }
        } else {
            line
        };
        let Some((name, value)) = line.split_once('=') else {
            if line == "API_ALCHEMY" {
                return Err(invalid(".env API_ALCHEMY requires =value"));
            }
            continue;
        };
        if name.trim() != "API_ALCHEMY" {
            continue;
        }
        if found.is_some() {
            return Err(invalid("duplicate API_ALCHEMY in .env"));
        }
        let value = value.trim();
        let key = if value.starts_with(['\'', '"']) {
            let quote = value.as_bytes()[0] as char;
            let (key, tail) = value[1..]
                .split_once(quote)
                .ok_or_else(|| invalid("unclosed API_ALCHEMY quote in .env"))?;
            if !tail.trim().is_empty() && !tail.trim().starts_with('#') {
                return Err(invalid("unexpected text after API_ALCHEMY in .env"));
            }
            key
        } else {
            match value.split_once('#') {
                Some((key, _)) => key.trim_end(),
                None => value,
            }
        };
        validate(key)?;
        found = Some(key);
    }
    Ok(found)
}

pub(in super::super) fn load(path: Option<&Path>) -> Result<(), Error> {
    if let Some(path) = path {
        let bytes =
            read_bounded_file(path, 1024).map_err(|_| invalid("cannot read Alchemy key file"))?;
        let key = std::str::from_utf8(&bytes)
            .map_err(|_| invalid("Alchemy key file must be UTF-8"))?
            .trim();
        validate(key)?;
        std::env::set_var("API_ALCHEMY", key);
    } else {
        load_dotenv()?;
    }
    if std::env::var_os("API_ALCHEMY").is_none() {
        return Err(invalid("set API_ALCHEMY in .env or your environment, or use --alchemy-key-file PATH; --demo needs no key"));
    }
    Ok(())
}
