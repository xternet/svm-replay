use super::*;

pub struct Progress {
    pub(in super::super) sender: Option<mpsc::Sender<&'static str>>,
    pub(in super::super) thread: Option<std::thread::JoinHandle<()>>,
}

pub(in super::super) fn animate_progress(stderr_is_terminal: bool, ci: bool) -> bool {
    stderr_is_terminal && !ci
}

impl Progress {
    pub fn finish(mut self, message: &'static str) {
        if let Some(sender) = self.sender.take() {
            if sender.send(message).is_err() {
                eprintln!("TERMINAL_IO: progress completion unavailable");
            }
        }
    }
    pub fn start(label: &'static str) -> Self {
        let animate = animate_progress(
            std::io::stderr().is_terminal(),
            std::env::var_os("CI").is_some(),
        );
        if !enabled() && !animate {
            return Self {
                sender: None,
                thread: None,
            };
        }
        if !animate {
            stage(label);
            return Self {
                sender: None,
                thread: None,
            };
        }
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let start = Instant::now();
            let mut tick = 0;
            loop {
                let mut out = std::io::stderr().lock();
                if write!(
                    out,
                    "\r  {} {} {}",
                    ['|', '/', '-', '\\'][tick % 4],
                    label,
                    style::stderr(&format!("({:.1}s)", start.elapsed().as_secs_f64()))
                )
                .and_then(|_| out.flush())
                .is_err()
                {
                    eprintln!("TERMINAL_IO: progress output unavailable");
                    break;
                }
                drop(out);
                tick += 1;
                match receiver.recv_timeout(Duration::from_millis(150)) {
                    Ok(message) => {
                        eprintln!(
                            "\r\x1b[2K  {}",
                            style::stderr(&progress_end(Some(message), start.elapsed()))
                        );
                        return;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        eprintln!(
                            "\r\x1b[2K  {}",
                            style::stderr(&progress_end(None, start.elapsed()))
                        );
                        return;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            eprintln!();
        });
        Self {
            sender: Some(sender),
            thread: Some(thread),
        }
    }
}

pub(in super::super) fn progress_end(message: Option<&str>, elapsed: Duration) -> String {
    match message {
        Some(message) => format!("✓ {message} ({:.1}s)", elapsed.as_secs_f64()),
        None => format!(
            "✗ Stage failed or was cancelled ({:.1}s)",
            elapsed.as_secs_f64()
        ),
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        // Disconnecting wakes the reader; no detached terminal thread survives.
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("TERMINAL_IO: progress thread failed");
            }
        }
    }
}
