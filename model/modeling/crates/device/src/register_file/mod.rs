mod bank;
mod image;
mod storage;

pub(crate) use bank::{MmioScalar, narrow_u32};
pub(crate) use image::PdmaRegisters;
#[cfg(debug_assertions)]
pub(crate) use storage::allocation_observation;

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}
