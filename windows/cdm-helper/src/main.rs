//! `nightglass-cdm <library>`: hosts Google's Widevine CDM and serves `nightglass_drm::protocol` on stdin and
//! stdout, so that a CDM that crashes or hangs takes only this process down. `--version` prints the CDM's version.

mod ffi;
mod host;
mod serve;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(library) = args.first().map(PathBuf::from) else {
        eprintln!("usage: nightglass-cdm <library> [--version]");
        return ExitCode::from(2);
    };
    let module = match host::Module::load(&library) {
        Ok(module) => module,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if args.iter().any(|a| a == "--version") {
        println!("{}", module.version());
        return ExitCode::SUCCESS;
    }
    serve::run(module)
}
