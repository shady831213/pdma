use pdma_model_contract::abi::header::{PdmaCHeaderExporter, PdmaHeaderSchemas};
use pdma_model_contract::translation::encode_id_value;
use pdma_model_contract::semantic::IrqVector;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = PdmaHeaderSchemas::resolve()?;
    let header = PdmaCHeaderExporter::new().render(&schemas);
    let vector = IrqVector::new(64);
    let encoded = encode_id_value(u128::try_from(vector.raw())?)?;
    println!(
        "header_bytes={};irq_vector={};translation={}",
        header.len(),
        vector.raw(),
        encoded.raw()
    );
    Ok(())
}
