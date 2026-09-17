//! PDMA software ring operations shared by Host and firmware.

use hardware_interface_runtime::{
    CpuContextError, CpuOwner, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use hardware_interface_schema::{Decode, Encode, RecordLayout};
use pdma_model_contract::{Ring, RingPointers};

mod config;
mod error;
mod io;
pub use config::RingConfig;
pub use error::SoftRingError;
pub use io::{MemoryRingIo, RegisterRingIo, RingCursorRegisters, SoftRingIo};

#[derive(Debug)]
struct PendingPush<E> {
    entry: E,
    cursor: u32,
    stored: bool,
}

/// CPU-owned software endpoint with resumable publication and cached consumption.
/// Keep the same ring and cursor transport throughout each retained operation.
#[derive(Debug)]
pub struct SoftRing<E> {
    config: RingConfig<E>,
    owner: CpuOwner,
    pending: Option<PendingPush<E>>,
    cached: Option<(u32, E)>,
}

impl<E> SoftRing<E>
where
    E: RecordLayout + Encode + Copy + Send + Sync,
    E::Encoded: Default + AsMut<[u8]> + AsRef<[u8]> + Decode<Decoded = E> + Send,
{
    /// Captures the active software CPU for validated ring storage.
    pub fn new(config: RingConfig<E>) -> Result<Self, CpuContextError> {
        Ok(Self {
            config,
            owner: CpuOwner::capture_active()?,
            pending: None,
            cached: None,
        })
    }

    /// Whether an earlier push still needs completion.
    pub const fn has_pending_push(&self) -> bool {
        self.pending.is_some()
    }

    /// Checks available storage without retaining an entry.
    pub async fn has_space<I: SoftRingIo>(&self, io: &I) -> Result<bool, SoftRingError<I::Error>> {
        self.owner
            .scope(async {
                if self.pending.is_some() {
                    return Err(SoftRingError::Busy);
                }
                let pointers = self.pointers(io).await?;
                Ok(pointers.pending() < self.config.capacity())
            })
            .await
            .map_err(SoftRingError::Cpu)?
    }

    /// Pushes one entry, or returns false without retaining it when full.
    /// Cancellation after acceptance retains the entry for `resume_push`.
    pub async fn try_push<I: SoftRingIo>(
        &mut self,
        io: &I,
        entry: E,
    ) -> Result<bool, SoftRingError<I::Error>> {
        let owner = self.owner.clone();
        owner
            .scope(async {
                if self.pending.is_some() {
                    return Err(SoftRingError::Busy);
                }
                let pointers = self.pointers(io).await?;
                if pointers.pending() == self.config.capacity() {
                    return Ok(false);
                }
                self.config
                    .advance_producer(pointers, 1)
                    .map_err(SoftRingError::Ring)?;
                self.pending = Some(PendingPush {
                    entry,
                    cursor: pointers.producer,
                    stored: false,
                });
                self.publish(io).await?;
                Ok(true)
            })
            .await
            .map_err(SoftRingError::Cpu)?
    }

    /// Completes the retained push, reconciling an already committed cursor write.
    pub async fn resume_push<I: SoftRingIo>(
        &mut self,
        io: &I,
    ) -> Result<(), SoftRingError<I::Error>> {
        let owner = self.owner.clone();
        owner
            .scope(self.publish(io))
            .await
            .map_err(SoftRingError::Cpu)?
    }

    async fn publish<I: SoftRingIo>(&mut self, io: &I) -> Result<(), SoftRingError<I::Error>> {
        let pending = self.pending.as_mut().ok_or(SoftRingError::Idle)?;
        let producer = io.read_producer().await.map_err(SoftRingError::Cursor)?;
        let next = pending.cursor.wrapping_add(1);
        if producer == next && pending.stored {
            self.pending = None;
            return Ok(());
        }
        if producer != pending.cursor {
            return Err(SoftRingError::CursorChanged);
        }
        let consumer = io.read_consumer().await.map_err(SoftRingError::Cursor)?;
        self.config
            .advance_producer(RingPointers { producer, consumer }, 1)
            .map_err(SoftRingError::Ring)?;
        if !pending.stored {
            let encoded = pending.entry.encode().map_err(SoftRingError::Codec)?;
            let address = self
                .config
                .entry(pending.cursor)
                .map_err(SoftRingError::Ring)?;
            io.memory()
                .write(
                    MemoryAddress::new(address),
                    MemoryAlignment::ONE,
                    encoded.as_ref(),
                )
                .map_err(SoftRingError::Memory)?;
            pending.stored = true;
        }
        io.write_producer(next)
            .await
            .map_err(SoftRingError::Cursor)?;
        self.pending = None;
        Ok(())
    }

    /// Returns and retains the next decoded entry without consuming it.
    /// Repeated calls return the cached entry even after a suspended consume.
    pub async fn peek<I: SoftRingIo>(
        &mut self,
        io: &I,
    ) -> Result<Option<E>, SoftRingError<I::Error>> {
        let owner = self.owner.clone();
        owner
            .scope(async {
                if let Some((_, entry)) = self.cached {
                    return Ok(Some(entry));
                }
                let pointers = self.pointers(io).await?;
                if pointers.is_empty() {
                    return Ok(None);
                }
                let address = self
                    .config
                    .entry(pointers.consumer)
                    .map_err(SoftRingError::Ring)?;
                let encoded: E::Encoded = io
                    .memory()
                    .read_typed(MemoryAddress::new(address), MemoryAlignment::ONE)
                    .map_err(SoftRingError::Memory)?;
                let entry = encoded.decode().map_err(SoftRingError::Codec)?;
                self.cached = Some((pointers.consumer, entry));
                Ok(Some(entry))
            })
            .await
            .map_err(SoftRingError::Cpu)?
    }

    /// Consumes the cached entry; retry reconciles a committed consumer write.
    pub async fn consume<I: SoftRingIo>(&mut self, io: &I) -> Result<(), SoftRingError<I::Error>> {
        let owner = self.owner.clone();
        owner
            .scope(async {
                let (cursor, _) = self.cached.ok_or(SoftRingError::Idle)?;
                let consumer = io.read_consumer().await.map_err(SoftRingError::Cursor)?;
                let next = cursor.wrapping_add(1);
                if consumer == next {
                    self.cached = None;
                    return Ok(());
                }
                if consumer != cursor {
                    return Err(SoftRingError::CursorChanged);
                }
                let producer = io.read_producer().await.map_err(SoftRingError::Cursor)?;
                self.config
                    .advance_consumer(RingPointers { producer, consumer }, 1)
                    .map_err(SoftRingError::Ring)?;
                io.write_consumer(next)
                    .await
                    .map_err(SoftRingError::Cursor)?;
                self.cached = None;
                Ok(())
            })
            .await
            .map_err(SoftRingError::Cpu)?
    }

    async fn pointers<I: SoftRingIo>(
        &self,
        io: &I,
    ) -> Result<RingPointers, SoftRingError<I::Error>> {
        let consumer = io.read_consumer().await.map_err(SoftRingError::Cursor)?;
        let producer = io.read_producer().await.map_err(SoftRingError::Cursor)?;
        let pointers = RingPointers { producer, consumer };
        self.config
            .advance_consumer(pointers, 0)
            .map_err(SoftRingError::Ring)
    }
}
