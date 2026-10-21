use super::*;
use std::io::Write;
use std::sync::mpsc;

pub type Sink = Arc<dyn Fn(ProgressEvent) + Send + Sync>;
pub(super) struct Shared {
    pub started: Instant,
    pub state: Mutex<State>,
    pub sink: Sink,
}
impl Shared {
    pub fn update(&self, change: impl FnOnce(&mut State, Duration) -> Option<Kind>) {
        match self.state.lock() {
            Ok(mut state) if state.outcome.is_none() => {
                let now = self.started.elapsed();
                if let Some(kind) = change(&mut state, now) {
                    (self.sink)(state.event(kind, now));
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("PROGRESS_STATE: {error}"),
        }
    }
}

/// One timer per job; drop always stops and joins it, including unwinding.
pub struct ProgressJob {
    shared: Arc<Shared>,
    previous: Option<Arc<Shared>>,
    stop: Option<mpsc::Sender<()>>,
    timer: Option<std::thread::JoinHandle<()>>,
    finished: bool,
    coordinator_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl ProgressJob {
    pub fn stderr(initial: Phase) -> Self {
        Self::start(
            initial,
            Arc::new(|event| {
                let result = (|| {
                    let bytes = serde_json::to_vec(&event).map_err(std::io::Error::other)?;
                    let mut stderr = std::io::stderr().lock();
                    stderr.write_all(&bytes)?;
                    stderr.write_all(b"\n")?;
                    stderr.flush()
                })();
                if let Err(error) = result {
                    eprintln!("PROGRESS_IO: {error}");
                }
            }),
        )
    }
    pub fn start(initial: Phase, sink: Sink) -> Self {
        Self::start_with_interval(initial, sink, HEARTBEAT_INTERVAL)
    }
    pub(super) fn start_with_interval(initial: Phase, sink: Sink, interval: Duration) -> Self {
        let shared = Arc::new(Shared {
            started: Instant::now(),
            state: Mutex::new(State::new(initial)),
            sink,
        });
        shared.update(|_, _| Some(Kind::Phase));
        let previous = CURRENT.with(|slot| slot.replace(Some(shared.clone())));
        let (stop, receiver) = mpsc::channel();
        let copy = shared.clone();
        let timer = std::thread::spawn(move || loop {
            match receiver.recv_timeout(interval) {
                Err(mpsc::RecvTimeoutError::Timeout) => copy.update(|_, _| Some(Kind::Heartbeat)),
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        });
        Self {
            shared,
            previous,
            stop: Some(stop),
            timer: Some(timer),
            finished: false,
            coordinator_thread: std::marker::PhantomData,
        }
    }
    pub fn finish(mut self, outcome: &str) {
        self.terminate(outcome);
    }
    fn terminate(&mut self, outcome: &str) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.stop.take();
        if let Some(timer) = self.timer.take() {
            if timer.join().is_err() {
                eprintln!("PROGRESS_TIMER: heartbeat thread failed");
            }
        }
        self.shared.update(|state, _| {
            state.outcome = Some(outcome.to_owned());
            Some(Kind::Terminal)
        });
        CURRENT.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}
impl Drop for ProgressJob {
    fn drop(&mut self) {
        self.terminate("ERROR");
    }
}
