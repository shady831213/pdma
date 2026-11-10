//! Exports the DMA UVM register model.
pub mod output;
/// Schema-backed application assembly.
pub mod uvm_regs;
const HELP: &str = "Usage: pdma-uvm-regs [--write FILE | --check FILE | --help]\nWithout arguments, writes the generated text to stdout.\n";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run(HELP, uvm_regs::generate)
}
