//! Prints the downstream-defined telemetry export report.

use std::process::ExitCode;

fn main() -> ExitCode {
    match downstream_exporter::output_line() {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
