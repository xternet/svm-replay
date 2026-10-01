use super::*;
pub(super) fn collect(
    reader: &mut impl Read,
    target: &mut Vec<u8>,
    other: usize,
    maximum: usize,
) -> Result<(), WorkerError> {
    let mut chunk = [0_u8; 16 * 1024];
    // Bounded rounds also service cancellation when a producer writes forever.
    for _ in 0..16 {
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(size) => {
                if size > maximum - target.len() - other {
                    return Err(WorkerError::new(
                        WorkerErrorCode::DiagnosticLimit,
                        format!("combined stdout/stderr exceeds {maximum} bytes"),
                    ));
                }
                target.extend_from_slice(&chunk[..size]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(WorkerError::io("read worker diagnostics", error)),
        }
    }
    Ok(())
}
