mod _0_arguments;
mod _10_run;
use _0_arguments as arguments;
mod _1_bundle;
use _1_bundle as bundle;
mod _2_debugger;
use _2_debugger as debugger;
mod _3_demo;
use _3_demo as demo;
pub(crate) mod _4_lifecycle;
pub(crate) use _4_lifecycle as lifecycle;
mod _5_report;
use _5_report as report;
mod _6_request;
use _6_request as request;
mod _7_sources;
use _7_sources as sources;
mod _8_terminal;
use _8_terminal as terminal;
mod _9_json;
use _9_json as json_output;

fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "__worker-owner") {
        if args.get(1).is_none_or(|arg| arg != "--") {
            eprintln!("worker owner requires -- separator");
            return std::process::ExitCode::FAILURE;
        }
        return match lifecycle::worker_owner::run(&args[2..]) {
            Ok(code) => std::process::ExitCode::from(code.clamp(0, 255) as u8),
            Err(error) => {
                eprintln!("WORKER_OWNER: {error}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    _10_run::run()
}
