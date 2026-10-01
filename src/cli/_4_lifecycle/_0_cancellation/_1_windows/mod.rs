use super::{CancellationToken, Error, SIGNAL_TOKEN};
pub fn install() -> Result<CancellationToken, Error> {
    use windows_sys::Win32::System::Console::*;
    unsafe extern "system" fn handler(event: u32) -> i32 {
        match event {
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT
            | CTRL_SHUTDOWN_EVENT => {
                if let Some(token) = SIGNAL_TOKEN.get() {
                    token.cancel();
                }
                1
            }
            _ => 0,
        }
    }
    let token = CancellationToken::new();
    SIGNAL_TOKEN
        .set(token.clone())
        .map_err(|_| Error::new("CLI_SIGNAL", "CLI cancellation is already installed"))?;
    if unsafe { SetConsoleCtrlHandler(Some(handler), 1) } == 0 {
        return Err(Error::new(
            "CLI_SIGNAL",
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok(token)
}
