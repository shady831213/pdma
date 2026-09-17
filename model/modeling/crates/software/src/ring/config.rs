use core::marker::PhantomData;
use hardware_interface_schema::RecordLayout;
use pdma_model_contract::{Ring, RingError};

/// Validated storage configuration selected by software, with no cursor state.
#[derive(Clone, Copy, Debug)]
pub struct RingConfig<Entry> {
    base: u64,
    capacity: u32,
    entry: PhantomData<fn() -> Entry>,
}

impl<Entry: RecordLayout> RingConfig<Entry> {
    /// Checks the complete ring extent before any entry address can be computed.
    pub fn new(base: u64, capacity: u32) -> Result<Self, RingError> {
        let config = Self {
            base,
            capacity,
            entry: PhantomData,
        };
        config.validate_geometry()?;
        Ok(config)
    }
}

impl<Entry: RecordLayout> Ring for RingConfig<Entry> {
    type Entry = Entry;

    fn base(&self) -> u64 {
        self.base
    }

    fn capacity(&self) -> u32 {
        self.capacity
    }
}
