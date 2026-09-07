mod image;
mod storage;

#[cfg(test)]
mod responses;

#[cfg(test)]
mod coherence;

#[cfg(test)]
mod lifecycle;

pub(crate) use image::PdmaRegisters;
#[cfg(test)]
pub(crate) use storage::allocation_observation;

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}

pub(crate) const fn narrow_u32(value: u64) -> u32 {
    let [byte0, byte1, byte2, byte3, _, _, _, _] = value.to_le_bytes();
    u32::from_le_bytes([byte0, byte1, byte2, byte3])
}
