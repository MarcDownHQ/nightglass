//! `nightglass-cdm <library> --version`: loads Google's Widevine CDM and prints its version.

mod ffi;
#[allow(dead_code)]
mod host;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(library) = args.first().map(PathBuf::from) else {
        eprintln!("usage: nightglass-cdm <library> --version");
        return ExitCode::from(2);
    };
    match host::Module::load(&library) {
        Ok(module) => {
            println!("{}", module.version());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
