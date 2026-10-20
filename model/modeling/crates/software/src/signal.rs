//! Signed 64-bit software signals over a supplied atomic memory capability.
//!
//! These are PDMA helpers, not the HSA runtime ABI. Storage allocation, initialization
//! and lifetime belong to the caller. There is no CPU ownership or interrupt registration.
//! Update methods return the observed old value, including operations whose HSA API
//! returns void. Waits return only a satisfying observation; there are no timeouts,
//! wait-state hints or spurious successful returns.

use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicReadOrder, AtomicUpdate,
    AtomicWriteOrder, MemoryAccess, MemoryAddress,
};
use pdma_model_contract::dma::{WaitCondition, WaitOrder};

/// Borrowed access to one naturally aligned, little-endian signed 64-bit signal.
///
/// The backend validates alignment and mapping during each indivisible operation.
/// Backend errors and unsupported atomic capability are preserved without fallback.
/// An error must leave storage unchanged under the [`AtomicOperation`] contract.
#[derive(Debug)]
pub struct Signal<'a, M: ?Sized> {
    memory: &'a M,
    address: MemoryAddress,
}

impl<'a, M: MemoryAccess + ?Sized> Signal<'a, M> {
    /// Borrows existing signal storage without allocating or accessing it.
    pub const fn new(memory: &'a M, address: MemoryAddress) -> Self {
        Self { memory, address }
    }

    /// Atomically observes the signed signal value.
    pub fn load(&self, order: AtomicReadOrder) -> Result<i64, AtomicAccessError<M::Error>> {
        self.access(AtomicOperation::Load(order))
    }

    /// Atomically replaces the signal value.
    pub fn store(
        &self,
        value: i64,
        order: AtomicWriteOrder,
    ) -> Result<(), AtomicAccessError<M::Error>> {
        self.access(AtomicOperation::Store {
            value: u64::from_le_bytes(value.to_le_bytes()),
            order,
        })?;
        Ok(())
    }

    /// Adds with two's-complement wrapping and returns the observed old value.
    pub fn add(&self, value: i64, order: AtomicOrder) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::Add(u64::from_le_bytes(value.to_le_bytes())),
            order,
        )
    }

    /// Subtracts with two's-complement wrapping and returns the observed old value.
    pub fn subtract(
        &self,
        value: i64,
        order: AtomicOrder,
    ) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::Subtract(u64::from_le_bytes(value.to_le_bytes())),
            order,
        )
    }

    /// Applies bitwise conjunction and returns the observed old value.
    pub fn and(&self, value: i64, order: AtomicOrder) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::And(u64::from_le_bytes(value.to_le_bytes())),
            order,
        )
    }

    /// Applies bitwise disjunction and returns the observed old value.
    pub fn or(&self, value: i64, order: AtomicOrder) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::Or(u64::from_le_bytes(value.to_le_bytes())),
            order,
        )
    }

    /// Applies bitwise exclusive disjunction and returns the observed old value.
    pub fn xor(&self, value: i64, order: AtomicOrder) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::Xor(u64::from_le_bytes(value.to_le_bytes())),
            order,
        )
    }

    /// Replaces the signal value and returns the observed old value.
    pub fn exchange(
        &self,
        value: i64,
        order: AtomicOrder,
    ) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::Exchange(u64::from_le_bytes(value.to_le_bytes())),
            order,
        )
    }

    /// Replaces only an equal current value and returns the observed value on either outcome.
    pub fn compare_exchange(
        &self,
        current: i64,
        new: i64,
        order: AtomicOrder,
    ) -> Result<i64, AtomicAccessError<M::Error>> {
        self.update(
            AtomicUpdate::CompareExchange {
                current: u64::from_le_bytes(current.to_le_bytes()),
                new: u64::from_le_bytes(new.to_le_bytes()),
            },
            order,
        )
    }

    /// Returns the observed signed value only when it satisfies the requested condition.
    pub fn try_wait(
        &self,
        condition: WaitCondition,
        compare: i64,
        order: WaitOrder,
    ) -> Result<Option<i64>, AtomicAccessError<M::Error>> {
        let order = match order {
            WaitOrder::Relaxed => AtomicReadOrder::Relaxed,
            WaitOrder::Acquire => AtomicReadOrder::Acquire,
        };
        let observed = self.load(order)?;
        Ok(condition.matches(observed, compare).then_some(observed))
    }

    /// Yields cooperatively until the signed condition is satisfied or access fails.
    ///
    /// # Cancel safety
    /// Dropping this future stops polling and does not modify storage or retain resources.
    pub async fn wait(
        &self,
        condition: WaitCondition,
        compare: i64,
        order: WaitOrder,
    ) -> Result<i64, AtomicAccessError<M::Error>>
    where
        M: Sync,
    {
        loop {
            let observed = self.try_wait(condition, compare, order)?;
            match observed {
                Some(observed) => return Ok(observed),
                None => tokio::task::yield_now().await,
            }
        }
    }

    fn update(
        &self,
        update: AtomicUpdate<u64>,
        order: AtomicOrder,
    ) -> Result<i64, AtomicAccessError<M::Error>> {
        self.access(AtomicOperation::Update { update, order })
    }

    fn access(&self, operation: AtomicOperation<u64>) -> Result<i64, AtomicAccessError<M::Error>> {
        self.memory
            .atomic64(self.address, operation)
            .map(|observed| i64::from_le_bytes(observed.to_le_bytes()))
    }
}
