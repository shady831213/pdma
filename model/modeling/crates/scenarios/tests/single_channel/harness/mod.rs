//! Platform assembly, actor driving and teardown for single-channel scenarios.

mod fault;
pub mod lifecycle;

use crate::{hardware, software};
use fault::{FaultBus, UnmappedRing};
use hardware_interface_memory::{
    MemoryAddress, SharedMemory, SharedMemoryAccess, SharedMemoryAllocator,
};
use hardware_interface_runtime::{InterruptRuntime, InterruptSummary, SoftwareCpu};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, CommandRegisters, EventRegisters};
use pdma_model_host::runtime::Model;
use std::{error::Error, sync::Arc};
use tokio::task::JoinError;

/// Scenario setup or execution infrastructure error.
pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

/// Test-side fault selection, never passed to the software program.
#[derive(Clone, Copy, Debug)]
pub enum FaultInjection {
    /// Normal memory and transport.
    None,
    /// Reject an error-event entry write.
    EventWrite,
    /// Reject the software command entry write.
    CommandWrite,
    /// Exhaust address space before software allocates its rings.
    AllocationExhausted,
}

/// Observable results retained after execution for case-owned assertions.
#[derive(Debug)]
pub struct Report {
    /// Software completion or its original failure.
    pub program: TestResult<software::Outcome>,
    /// Joined cancellation of the persistent hardware worker after software finishes.
    pub worker: JoinError,
    /// Completed interrupt delivery accounting.
    pub summary: InterruptSummary,
    /// Final hardware interrupt level.
    pub irq: bool,
    /// Backing memory for independent payload and allocation checks.
    pub storage: Arc<SharedMemory>,
}

/// Assemble and drive one complete software program with the requested test fault.
///
/// # Panics
/// Panics if platform setup allocates application buffers before software starts.
pub async fn run(injection: FaultInjection, input: software::Input<'_>) -> TestResult<Report> {
    let storage = Arc::new(SharedMemory::new());
    let first_address = match injection {
        FaultInjection::AllocationExhausted => u64::MAX,
        FaultInjection::None | FaultInjection::EventWrite | FaultInjection::CommandWrite => 0x1000,
    };
    let allocator =
        SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(first_address));
    let faulted = Arc::new(UnmappedRing::new(allocator.memory_access()));
    let access = SharedMemoryAccess::from(Arc::clone(&faulted));
    let (device, hardware) = hardware::assemble(access.clone())?;
    let model = Model::new(device);
    let channel_base = ChannelId::new(0)
        .register_offset(1)
        .ok_or("channel zero missing")?;
    let base_register = match injection {
        FaultInjection::EventWrite => Some(u32::try_from(
            channel_base + ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
        )?),
        FaultInjection::CommandWrite => Some(u32::try_from(
            channel_base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
        )?),
        FaultInjection::None | FaultInjection::AllocationExhausted => None,
    };
    let bus = FaultBus {
        bus: model.clone(),
        base_register,
        memory: faulted,
    };
    let cpu = Arc::new(SoftwareCpu::new());
    assert!(
        storage.regions()?.is_empty(),
        "buffers must be allocated by the running software"
    );
    let runtime = InterruptRuntime::new(cpu, model.clone());
    let execution = runtime.run_main(async |interrupts| {
        software::main(
            software::Ports {
                bus,
                memory: access,
                allocator,
                interrupts,
            },
            input,
        )
        .await
    });
    let ((program, summary), worker) = lifecycle::run(
        hardware::drive(model.clone(), hardware),
        Box::pin(async { execution.await.map_err(Into::into) }),
    )
    .await?;
    Ok(Report {
        program,
        worker,
        summary,
        irq: model.irq(),
        storage,
    })
}
