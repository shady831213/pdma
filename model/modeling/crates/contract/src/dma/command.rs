mod codec;
mod header;
use super::operations::{
    AtomicCommand, CopyCommand, FenceCommand, FillCommand, SemaphoreCommand, TimestampCommand,
    TrapCommand, WaitCommand, WriteCommand,
};
use hardware_interface_schema::union_schema;
pub use header::*;

union_schema! {
    #[short_description = "Complete copy-engine command union"]
    #[description = "Untagged 96-byte union of complete command records. PDMA selects a member using the shared header opcode at byte zero."]
    pub union Command { cases: {
        Copy(CopyCommand), Write(WriteCommand), Fill(FillCommand), Wait(WaitCommand),
        Atomic(AtomicCommand), Semaphore(SemaphoreCommand), Fence(FenceCommand), Trap(TrapCommand), Timestamp(TimestampCommand),
    } }
}
