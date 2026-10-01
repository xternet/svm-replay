use super::{CancellationToken, Error, SIGNAL_TOKEN};
extern "C" fn cancel(_: libc::c_int) {
    // Initialized before either handler is installed. OnceLock::get is
    // nonblocking; CancellationToken::cancel only performs an AtomicBool store.
    if let Some(token) = SIGNAL_TOKEN.get() {
        token.cancel();
    }
}

pub fn install() -> Result<CancellationToken, Error> {
    let token = CancellationToken::new();
    SIGNAL_TOKEN
        .set(token.clone())
        .map_err(|_| Error::new("CLI_SIGNAL", "CLI cancellation is already installed"))?;
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = cancel as *const () as usize;
        action.sa_flags = libc::SA_RESTART;
        if libc::sigemptyset(&mut action.sa_mask) != 0 {
            return Err(Error::new(
                "CLI_SIGNAL",
                format!(
                    "initialize signal mask: {}",
                    std::io::Error::last_os_error()
                ),
            ));
        }
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(Error::new(
                    "CLI_SIGNAL",
                    format!(
                        "install signal {signal}: {}",
                        std::io::Error::last_os_error()
                    ),
                ));
            }
        }
    }
    Ok(token)
}
