//! Deterministic read/write engine execution for scheduled copy work.

use crate::ModelResult;
use crate::desc::ScheduledCopy;
use crate::memory::Memory;

/// Copy engine that performs one read followed by one write per scheduled item.
#[derive(Debug, Default, Clone, Copy)]
pub struct CopyEngine;

impl CopyEngine {
    /// Executes a scheduled copy through checked memory operations.
    pub fn execute(memory: &mut Memory, work: &ScheduledCopy) -> ModelResult<()> {
        for chunk in work.chunks() {
            let bytes = memory.read_checked(chunk.src(), chunk.src_offset(), chunk.len())?;
            memory.write_checked(chunk.dst(), chunk.dst_offset(), &bytes)?;
        }
        Ok(())
    }
}
