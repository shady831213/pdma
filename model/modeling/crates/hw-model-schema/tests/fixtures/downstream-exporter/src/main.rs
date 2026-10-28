//! Prints the downstream-defined telemetry export report.

use std::process::ExitCode;

fn main() -> ExitCode {
    downstream_exporter::output_line().map_or(ExitCode::FAILURE, |line| {
        println!("{line}");
        ExitCode::SUCCESS
    })
}
