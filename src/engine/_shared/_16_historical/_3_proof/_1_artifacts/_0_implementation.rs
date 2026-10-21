use super::*;

pub(in super::super) fn read(
    path: &Path,
    expected: &str,
    maximum: usize,
    budget: &ExecutionBudget,
) -> Result<Vec<u8>, Error> {
    budget.check().map_err(protocol_error)?;
    Digest::new(expected).map_err(|_| invalid("malformed session artifact pin"))?;
    let bytes = read_bounded_file(path, maximum).map_err(protocol_error)?;
    budget.check().map_err(protocol_error)?;
    if Digest::of(&bytes).as_str() != expected {
        return Err(Error::new(
            "SESSION_INTEGRITY",
            format!("artifact SHA-256 differs: {}", path.display()),
        ));
    }
    Ok(bytes)
}

pub(in super::super) fn artifact(
    root: &Path,
    pin: &Value,
    maximum: usize,
    budget: &ExecutionBudget,
) -> Result<Vec<u8>, Error> {
    let relative = Path::new(text(&pin["file"])?);
    check(
        !relative.as_os_str().is_empty()
            && relative
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "session artifact path must be relative and cannot traverse",
    )?;
    let root = root
        .canonicalize()
        .map_err(|e| invalid(format!("session artifact root: {e}")))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| invalid(format!("session artifact path: {e}")))?;
    check(
        path.starts_with(&root),
        "session artifact escapes receipt directory",
    )?;
    read(&path, text(&pin["sha256"])?, maximum, budget)
}
