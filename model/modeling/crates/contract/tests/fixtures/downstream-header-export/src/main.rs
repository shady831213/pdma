use pdma_model_contract::abi::header::{PdmaCHeaderExporter, PdmaHeaderSchemas};
use pdma_model_contract::semantic::IrqVector;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = PdmaHeaderSchemas::resolve()?;
    let header = PdmaCHeaderExporter::new().render(&schemas);
    let vector = IrqVector::new(64);
    let encoded = u64::try_from(vector.raw())?;
    println!(
        "header_bytes={};irq_vector={};translation={}",
        header.len(),
        vector.raw(),
        encoded
    );
    Ok(())
}
