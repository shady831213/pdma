use core::marker::PhantomData;
use hw_model_schema::SchemaCodec;
use pdma_model_contract::ring::{Ring, RingError};

/// Validated storage configuration selected by software, with no cursor state.
#[derive(Clone, Copy, Debug)]
pub struct RingConfig<Entry> {
    base: u64,
    capacity: u32,
    entry: PhantomData<fn() -> Entry>,
}

impl<Entry: SchemaCodec> RingConfig<Entry> {
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

impl<Entry: SchemaCodec> Ring for RingConfig<Entry> {
    type Entry = Entry;

    fn base(&self) -> u64 {
        self.base
    }

    fn capacity(&self) -> u32 {
        self.capacity
    }
}
