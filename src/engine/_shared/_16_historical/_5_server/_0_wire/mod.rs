use super::*;
mod _0_read;
mod _1_write;
pub(super) use _0_read::read;
pub(super) use _1_write::write;

fn failure(status: u16, message: &str) -> Error {
    Error::new("SESSION_HTTP", message).with_details(json!({"httpStatus":status}))
}
fn retry(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
    )
}
