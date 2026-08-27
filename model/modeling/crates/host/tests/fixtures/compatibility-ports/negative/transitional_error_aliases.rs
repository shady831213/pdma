use pdma_model_device::{BusControlError, MmioBatchError};

fn main() {
    let _ = std::mem::size_of::<BusControlError>();
    let _ = std::mem::size_of::<MmioBatchError>();
}
