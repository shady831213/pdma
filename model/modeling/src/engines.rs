//! Deterministic read/write engine execution for scheduled copy work.

use crate::ModelResult;
use crate::desc::ScheduledCopy;
use crate::memory::Memory;

/// Copy engine that performs one read followed by one write per scheduled item.
#[derive(Debug, Default, Clone, Copy)]
pub struct CopyEngine;

impl CopyEngine {
    /// Executes a scheduled copy through checked memory operations.
    pub fn execute(memory: &mut Memory, work: ScheduledCopy) -> ModelResult<()> {
        let request = work.request();
        let bytes = memory.read_checked(request.src(), 0, request.len())?;
        memory.write_checked(request.dst(), 0, &bytes)
    }
}
