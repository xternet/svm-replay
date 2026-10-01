use super::*;

pub(in super::super) struct InputWriter<'a> {
    pub(in super::super) file: File,
    pub(in super::super) maximum: usize,
    pub(in super::super) written: usize,
    pub(in super::super) budget: &'a ExecutionBudget,
    pub(in super::super) limit_hit: bool,
}

impl Write for InputWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.budget.check().map_err(io::Error::other)?;
        if bytes.len() > self.maximum - self.written {
            self.limit_hit = true;
            return Err(io::Error::other("input byte limit exceeded"));
        }
        let written = self.file.write(bytes)?;
        self.written += written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub(in super::super) fn write_input(
    path: &Path,
    fixture: &Value,
    maximum: usize,
    budget: &ExecutionBudget,
) -> Result<(), WorkerError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|error| WorkerError::io("create fixture", error))?;
    let mut writer = InputWriter {
        file,
        maximum,
        written: 0,
        budget,
        limit_hit: false,
    };
    if let Err(error) = serde_json::to_writer(&mut writer, fixture) {
        budget.check()?;
        return Err(WorkerError::new(
            if writer.limit_hit {
                WorkerErrorCode::InputLimit
            } else {
                WorkerErrorCode::Io
            },
            format!("serialize worker fixture: {error}"),
        ));
    }
    writer
        .flush()
        .map_err(|error| WorkerError::io("flush fixture", error))
}

pub(crate) fn check_output(path: &Path, maximum: usize) -> Result<(), WorkerError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(WorkerError::io("monitor output file", error)),
    };
    if !metadata.is_file() {
        return Err(WorkerError::new(
            WorkerErrorCode::Io,
            format!("worker output {} is not a regular file", path.display()),
        ));
    }
    if metadata.len() > maximum as u64 {
        return Err(WorkerError::new(
            WorkerErrorCode::OutputLimit,
            format!(
                "worker output {} has {} bytes, limit {maximum}",
                path.display(),
                metadata.len()
            ),
        ));
    }
    Ok(())
}
