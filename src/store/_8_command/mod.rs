use std::io::{Read, Write};
use std::path::Path;
use svm_replay_store::{parse_request, Store, StoreError, MAX_REQUEST_BYTES};

fn run() -> Result<serde_json::Value, StoreError> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 2 {
        return Err(StoreError::new(
            "INVALID_REQUEST",
            "usage: svm-replay-store <store-root>; send one versioned JSON request on stdin",
        ));
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let request = parse_request(&bytes)?;
    let mut store = Store::open(Path::new(&args[1]))?;
    store.execute(request)
}

fn main() {
    let (response, success) = match run() {
        Ok(value) => (value, true),
        Err(e) => (e.response(), false),
    };
    let output =
        serde_json::to_vec(&response).expect("JSON response contains only serializable values");
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(&output)
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush())
    {
        eprintln!("OUTPUT_ERROR: {e}");
        std::process::exit(1);
    }
    if !success {
        std::process::exit(1);
    }
}
