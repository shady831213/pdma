//! Exports the DMA SV records.
pub mod output;
/// Schema-backed application assembly.
pub mod sv_records;
const HELP: &str = "Usage: pdma-sv-records [--write FILE | --check FILE | --help]\nWithout arguments, writes the generated text to stdout.\n";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run(HELP, sv_records::generate)
}
