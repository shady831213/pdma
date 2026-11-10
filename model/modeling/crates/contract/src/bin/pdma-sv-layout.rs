//! Exports the DMA SV register layout.
pub mod output;
/// Schema-backed application assembly.
pub mod sv_layout;
const HELP: &str = "Usage: pdma-sv-layout [--write FILE | --check FILE | --help]\nWithout arguments, writes the generated text to stdout.\n";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run(HELP, sv_layout::generate)
}
