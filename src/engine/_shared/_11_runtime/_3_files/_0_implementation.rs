use super::*;

pub(in super::super) fn open_regular(path: &Path) -> Result<File, WorkerError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| WorkerError::io(&format!("open {}", path.display()), error))?;
    if !file
        .metadata()
        .map_err(|error| WorkerError::io("file metadata", error))?
        .is_file()
    {
        return Err(WorkerError::new(
            WorkerErrorCode::Io,
            format!("{} is not a regular file", path.display()),
        ));
    }
    Ok(file)
}

pub fn file_sha256(path: &Path) -> Result<String, WorkerError> {
    hash_file(path, None)
}

pub(in super::super) fn hash_file(
    path: &Path,
    budget: Option<&ExecutionBudget>,
) -> Result<String, WorkerError> {
    let mut file = open_regular(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        if let Some(budget) = budget {
            budget.check()?;
        }
        let size = file
            .read(&mut bytes)
            .map_err(|error| WorkerError::io("hash executable", error))?;
        if size == 0 {
            break;
        }
        hash.update(&bytes[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(in super::super) fn verify_pin(
    path: &Path,
    expected: &str,
    budget: Option<&ExecutionBudget>,
) -> Result<(), WorkerError> {
    if !path.is_absolute()
        || expected.len() != 64
        || !expected
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WorkerError::new(
            WorkerErrorCode::InvalidConfig,
            "worker requires absolute executable path and lowercase SHA-256 pin",
        ));
    }
    let actual = hash_file(path, budget)?;
    if actual != expected {
        return Err(WorkerError::new(
            WorkerErrorCode::Integrity,
            format!(
                "{}: expected SHA-256 {expected}, observed {actual}",
                path.display()
            ),
        ));
    }
    Ok(())
}

pub fn read_bounded_file(path: &Path, maximum: usize) -> Result<Vec<u8>, WorkerError> {
    if maximum == 0 {
        return Err(WorkerError::new(
            WorkerErrorCode::InvalidConfig,
            "file limit must be positive",
        ));
    }
    let mut file = open_regular(path)?;
    let size = file
        .metadata()
        .map_err(|error| WorkerError::io("result metadata", error))?
        .len();
    if size > maximum as u64 {
        return Err(WorkerError::new(
            WorkerErrorCode::OutputLimit,
            format!("{} exceeds {maximum} bytes: {size}", path.display()),
        ));
    }
    let mut output = Vec::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let size = file
            .read(&mut chunk)
            .map_err(|error| WorkerError::io("read worker output", error))?;
        if size == 0 {
            return Ok(output);
        }
        if size > maximum - output.len() {
            return Err(WorkerError::new(
                WorkerErrorCode::OutputLimit,
                "worker output grew beyond byte limit",
            ));
        }
        output.extend_from_slice(&chunk[..size]);
    }
}
