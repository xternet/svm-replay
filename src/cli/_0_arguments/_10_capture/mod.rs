use super::*;

pub(super) fn captured_pair(
    path: Option<PathBuf>,
    pin: Option<String>,
) -> Result<Option<(PathBuf, Digest)>, Error> {
    match (path, pin) {
        (Some(path), Some(pin)) => Ok(Some((path, Digest::new(pin)?))),
        (None, None) => Ok(None),
        _ => Err(Error::new(
            "INVALID_REQUEST",
            "captured source path and pin must be paired",
        )),
    }
}
