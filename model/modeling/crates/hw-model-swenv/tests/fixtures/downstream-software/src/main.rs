//! Prints the standalone runtime-consumer report.

use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match downstream_software::run_flow().await {
        Ok(report) => {
            println!("{}", report.happy_line);
            println!("{}", report.adverse_line);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
