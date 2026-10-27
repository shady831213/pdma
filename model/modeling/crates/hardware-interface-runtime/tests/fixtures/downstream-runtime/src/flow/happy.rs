use super::{FlowError, FlowResult, HappyFlow, require};
use crate::{run_happy_dispatch, run_register};
use hardware_interface_memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use hir::TraceBuffer;

pub(super) async fn run() -> FlowResult<HappyFlow> {
    let trace = TraceBuffer::<(usize, &'static str)>::new();
    let (memory, region, alignment) = memory_flow(&trace)?;
    run_register(&trace).await?;
    let deliveries = run_happy_dispatch(&trace).await?;
    Ok(HappyFlow {
        memory,
        region,
        alignment,
        trace,
        deliveries,
    })
}

fn memory_flow(
    trace: &TraceBuffer<(usize, &'static str)>,
) -> FlowResult<(SharedMemory, MemoryRegion, MemoryAlignment)> {
    let memory = SharedMemory::new();
    let alignment = MemoryAlignment::try_new(64)?;
    let region = MemoryRegion::try_new(MemoryAddress::new(0x5000), 64, alignment)?;
    memory.add_regions(&[region])?;
    let mut descriptor = [0x5a; 64];
    let Some((first, remaining)) = descriptor.split_first_mut() else {
        return Err(Box::new(FlowError("descriptor is unexpectedly empty")));
    };
    *first = 0x11;
    let Some(last) = remaining.last_mut() else {
        return Err(Box::new(FlowError("descriptor has no last byte")));
    };
    *last = 0xee;
    memory.write(MemoryAddress::new(0x5000), alignment, &descriptor)?;
    trace.push_with(|sequence| (sequence, "memory-write"));
    let mut output = [0_u8; 64];
    memory.read(MemoryAddress::new(0x5000), alignment, &mut output)?;
    trace.push_with(|sequence| (sequence, "memory-read"));
    require(
        output == descriptor,
        "memory read did not match literal input",
    )?;
    Ok((memory, region, alignment))
}
