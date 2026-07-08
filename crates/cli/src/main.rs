//! CLI entry point for the webcam -> Spout/Syphon sharing tool.

mod args;
mod run;
mod select;

use std::process::ExitCode;

use clap::Parser;

use args::Args;
use run::run;

fn main() -> ExitCode {
    let args = Args::parse();

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
