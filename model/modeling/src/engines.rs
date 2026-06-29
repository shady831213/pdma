//! Deterministic read/write engine execution for scheduled copy work.

use crate::ModelError;
use crate::desc::{ScheduledCopy, ScheduledVectorAdd};
use crate::event::{EventErrorKind, MemoryFault};
use crate::memory::Memory;

/// Copy engine that performs one read followed by one write per scheduled item.
#[derive(Debug, Default, Clone, Copy)]
pub struct CopyEngine;

/// Vector-add engine that performs two reads, byte-wise wrapping addition, then one write.
#[derive(Debug, Default, Clone, Copy)]
pub struct VectorAddEngine;

/// Error returned by copy engine execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    /// A memory access fault that should be reported through the event path.
    Memory(MemoryFault),
    /// A non-memory model error that should propagate directly.
    Model(ModelError),
}

impl CopyEngine {
    /// Executes a scheduled copy through checked memory operations.
    pub fn execute(memory: &mut Memory, work: &ScheduledCopy) -> Result<(), EngineError> {
        for chunk in work.chunks() {
            let bytes = memory
                .read_checked(chunk.src(), chunk.src_offset(), chunk.len())
                .map_err(|error| memory_fault(error, chunk.src()))?;
            memory
                .write_checked(chunk.dst(), chunk.dst_offset(), &bytes)
                .map_err(|error| memory_fault(error, chunk.dst()))?;
        }
        Ok(())
    }
}

impl VectorAddEngine {
    /// Executes a scheduled vector add through checked memory operations.
    pub fn execute(memory: &mut Memory, work: ScheduledVectorAdd) -> Result<(), EngineError> {
        let request = work.request();
        let src_a = memory
            .read_checked(request.src_a(), request.src_a_offset(), request.len())
            .map_err(|error| memory_fault(error, request.src_a()))?;
        let src_b = memory
            .read_checked(request.src_b(), request.src_b_offset(), request.len())
            .map_err(|error| memory_fault(error, request.src_b()))?;
        let sum = src_a
            .iter()
            .zip(src_b.iter())
            .map(|(left, right)| left.wrapping_add(*right))
            .collect::<Vec<_>>();
        memory
            .write_checked(request.dst(), request.dst_offset(), &sum)
            .map_err(|error| memory_fault(error, request.dst()))
    }
}

pub(crate) const fn memory_fault(error: ModelError, buffer: crate::ids::BufferId) -> EngineError {
    let error_kind = match error {
        ModelError::UnknownBuffer(_) => EventErrorKind::UnknownBuffer,
        ModelError::OutOfBounds { len, end, .. } => EventErrorKind::OutOfBounds { len, end },
        ModelError::RangeOverflow => EventErrorKind::RangeOverflow,
        ModelError::ZeroCapacity
        | ModelError::RingFull
        | ModelError::UnknownQueue(_)
        | ModelError::UnknownCompletionRing(_)
        | ModelError::DriverQueueFull
        | ModelError::StepBudgetExhausted { .. } => return EngineError::Model(error),
    };
    EngineError::Memory(MemoryFault::new(error, buffer, error_kind))
}
