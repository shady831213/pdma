mod bank;
mod image;
mod irq;
mod queue;
mod storage;

pub(crate) use bank::{MmioScalar, narrow_u32};
pub(crate) use image::PdmaRegisters;
pub(crate) use queue::QueueStaging;
#[cfg(debug_assertions)]
pub(crate) use storage::allocation_observation;

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}
