use iso_extract_jpegs::cli::{self, CliAction};
use iso_extract_jpegs::{extract_jpegs, report};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::parse_args(env::args_os().skip(1)) {
        Ok(CliAction::Help) => {
            print!("{}", cli::usage());
            ExitCode::SUCCESS
        }
        Ok(CliAction::Version) => {
            println!("iso-extract-jpegs {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(CliAction::Run(config)) => {
            let verbose = config.verbose;
            match extract_jpegs(config) {
                Ok(summary) => {
                    if let Err(error) = report::print_human(&summary, verbose) {
                        eprintln!("error: could not print summary: {error}");
                        return ExitCode::from(1);
                    }

                    if summary.failed > 0 {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(error.exit_code())
                }
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!();
            eprint!("{}", cli::usage());
            ExitCode::from(error.exit_code())
        }
    }
}
