use super::*;
pub(crate) fn write_final(value: &Value) -> Result<(), Error> {
    if value.pointer("/error/details/partialStdoutLine") == Some(&Value::Bool(true)) {
        return Err(Error::new(
            "CLI_OUTPUT",
            "cannot append receipt to incomplete stdout frame",
        ));
    }
    let mut line = crate::json_output::encode(value)?;
    if line.len() > 512 * 1024 * 1024 {
        return Err(Error::new(
            "CLI_OUTPUT_LIMIT",
            "final receipt exceeds output limit",
        ));
    }
    line.push(b'\n');
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let writer = thread::Builder::new()
        .name("svm-cli-output".into())
        .spawn(move || {
            let stdout = io::stdout();
            let mut stdout = stdout.lock();
            let mut offset = 0;
            while offset < line.len() {
                if stopped.load(Ordering::Acquire) {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "stdout deadline exceeded",
                    ));
                }
                let count = stdout.write(&line[offset..])?;
                if count == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "stdout write returned zero",
                    ));
                }
                offset += count;
            }
            stdout.flush()
        })
        .map_err(|e| Error::new("CLI_IO", e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut cancel_error = None;
    while !writer.is_finished() {
        if Instant::now() >= deadline {
            stop.store(true, Ordering::Release);
            if unsafe { CancelSynchronousIo(writer.as_raw_handle()) } == 0 {
                let error = io::Error::last_os_error();
                // The writer may be between writes or have just completed.
                if error.raw_os_error() != Some(ERROR_NOT_FOUND as i32) {
                    eprintln!("CLI_IO: cancel stdout: {error}");
                    cancel_error = Some(error.to_string());
                }
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    let result = writer
        .join()
        .map_err(|_| Error::new("CLI_IO", "output thread panicked"))?;
    if let Some(error) = cancel_error {
        return Err(Error::new("CLI_IO", error));
    }
    result.map_err(|e| Error::new("CLI_IO", e.to_string()))
}
