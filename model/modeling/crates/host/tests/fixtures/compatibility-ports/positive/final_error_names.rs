use pdma_model_device::{PdmaBatchError, PdmaControlError};

fn main() {
    let _ = std::mem::size_of::<PdmaBatchError>();
    let _ = std::mem::size_of::<PdmaControlError>();
}
