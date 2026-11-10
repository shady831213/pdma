//! Exports the DMA Veryl layout.
pub mod output;
/// Schema-backed application assembly.
pub mod rtl_layout;
const HELP: &str = "Usage: pdma-rtl-layout [--write FILE | --check FILE | --help]\nWithout arguments, writes the generated text to stdout.\n";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run(HELP, rtl_layout::generate)
}
