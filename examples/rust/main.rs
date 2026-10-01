//! Minimal Rust host: bind the coordinator to this binary; use the pinned CLI
//! only as the child-process owner. No TypeScript coordinator or live provider.
use std::{env, fs, path::PathBuf};
use svm_replay_engine::{
    shared::runtime::{file_sha256, CancellationToken, ProcessOwner},
    simulate_prepared, CacheMode, Config, PreparedRequest,
};
use svm_replay_protocol::Digest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: rust-consumer PREPARED_REQUEST CATALOG PIN CLI_OWNER DATA_DIR".into());
    }
    let owner = PathBuf::from(&args[3]).canonicalize()?;
    let config = Config {
        catalog_path: PathBuf::from(&args[1]).canonicalize()?,
        catalog_sha256: Digest::new(&args[2])?,
        data_dir: PathBuf::from(&args[4]),
        owner: ProcessOwner {
            sha256: file_sha256(&owner)?,
            executable: owner,
        },
        cache: CacheMode::Off,
        trace: None,
    };
    let request = PreparedRequest::parse(&fs::read(&args[0])?)?;
    let receipt = simulate_prepared(request, &config, CancellationToken::new())?;
    println!("{receipt}");
    if receipt["outcome"] != "COMPLETED" {
        return Err("historical simulation did not complete; see typed receipt".into());
    }
    Ok(())
}
