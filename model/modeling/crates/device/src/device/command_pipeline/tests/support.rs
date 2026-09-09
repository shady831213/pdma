use std::num::NonZeroUsize;

use crate::test_support::{self, TestQueue};
use hardware_interface_pipeline::{
    ImmediateExecutor, Operation, RunError, RunOutcome, StepExecutor,
};
use pdma_model_contract::abi::registers::IrqSourceRegisters;
use pdma_model_contract::semantic::ResourceConfig;

use crate::device::{DeviceCore, ModelStepStatus};
use crate::{AsyncStallReason, ModelError};
use pdma_model_contract::abi::DmaRegion;

mod descriptor;
mod observation;

pub(super) use observation::{cursors, drive_to_complete, drive_to_pending, read, trace_kinds};

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Copy,
    Strided,
    Broadcast,
    VectorAdd,
    UnknownBuffer,
    RangeOverflow,
    OutOfBounds,
}

impl Case {
    pub(super) const ALL: [Self; 7] = [
        Self::Copy,
        Self::Strided,
        Self::Broadcast,
        Self::VectorAdd,
        Self::UnknownBuffer,
        Self::RangeOverflow,
        Self::OutOfBounds,
    ];
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PipelineRunOutcome {
    Progressed,
    Pending(AsyncStallReason),
    Complete(Result<ModelStepStatus, ModelError>),
    Exhausted,
    BudgetExhausted,
}

#[derive(Debug)]
pub(super) struct Fixture {
    pub(super) core: DeviceCore,
    pub(super) allocator: hardware_interface_runtime::SharedMemoryAllocator,
    pub(super) operation: Operation<DeviceCore, ()>,
}

pub(super) struct Prepared {
    pub(super) fixture: Fixture,
    pub(super) queue: TestQueue,
    pub(super) destinations: [DmaRegion; 2],
}

impl Fixture {
    pub(super) fn new(resources: ResourceConfig) -> Self {
        let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            hardware_interface_runtime::MemoryAddress::new(0x1_0000),
        );
        Self {
            core: DeviceCore::new(allocator.memory_access(), resources),
            allocator,
            operation: Operation::new(()),
        }
    }

    pub(super) fn restart(&mut self) {
        self.operation = Operation::new(());
    }

    pub(super) const fn set_resources(&mut self, resources: ResourceConfig) {
        self.core.command_pipeline.execution =
            crate::device::execution::ExecutionUnit::new(resources);
    }

    pub(super) const fn fetch_count(&self) -> usize {
        self.core.native_steps()
    }

    pub(super) fn step(&mut self) -> PipelineRunOutcome {
        normalize(&StepExecutor::new().step(&mut self.core, &mut self.operation, &mut ()))
    }

    pub(super) fn immediate(&mut self) -> PipelineRunOutcome {
        normalize(
            &ImmediateExecutor::new(NonZeroUsize::new(16).expect("nonzero budget")).run(
                &mut self.core,
                &mut self.operation,
                &mut (),
            ),
        )
    }
}

pub(super) fn normalize(
    outcome: &Result<RunOutcome<ModelStepStatus, ModelError, AsyncStallReason>, RunError>,
) -> PipelineRunOutcome {
    match outcome {
        Ok(RunOutcome::Progressed) => PipelineRunOutcome::Progressed,
        Ok(RunOutcome::Pending(wait)) => PipelineRunOutcome::Pending(*wait),
        Ok(RunOutcome::Complete(result)) => PipelineRunOutcome::Complete(*result),
        Err(RunError::Exhausted) => PipelineRunOutcome::Exhausted,
        Err(RunError::BudgetExhausted) => PipelineRunOutcome::BudgetExhausted,
    }
}

pub(super) fn prepare(case: Case) -> Prepared {
    prepare_with_resources(case, ResourceConfig::default())
}

pub(super) fn prepare_with_resources(case: Case, resources: ResourceConfig) -> Prepared {
    let mut fixture = Fixture::new(resources);
    fixture.core.enable_tracing();
    let source_a = crate::test_support::allocate(&fixture.allocator, 8);
    let source_b = crate::test_support::allocate(&fixture.allocator, 8);
    let destination_a = crate::test_support::allocate(&fixture.allocator, 8);
    let destination_b = crate::test_support::allocate(&fixture.allocator, 8);
    crate::memory::write(
        &fixture.core.memory,
        source_a.base(),
        &[1, 2, 3, 4, 5, 6, 7, 8],
    )
    .expect("source A bytes");
    crate::memory::write(
        &fixture.core.memory,
        source_b.base(),
        &[8, 7, 6, 5, 4, 3, 2, 1],
    )
    .expect("source B bytes");
    let queue = test_support::configure(&mut fixture.core, &fixture.allocator, 0, [4, 1, 1]);
    let command = descriptor::command(
        &fixture,
        case,
        [source_a, source_b, destination_a, destination_b],
    );
    test_support::submit(&mut fixture.core, queue, command);
    test_support::doorbell(&mut fixture.core, queue);
    test_support::irq_write(
        &mut fixture.core,
        queue,
        0,
        IrqSourceRegisters::ENABLED.offset(),
        1,
    );
    test_support::irq_write(
        &mut fixture.core,
        queue,
        0,
        IrqSourceRegisters::MASKED.offset(),
        0,
    );
    Prepared {
        fixture,
        queue,
        destinations: [destination_a, destination_b],
    }
}

pub(super) fn prepared_pair() -> Prepared {
    let mut prepared = prepare_with_resources(
        Case::Copy,
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let source = crate::test_support::allocate(&prepared.fixture.allocator, 8);
    crate::memory::write(
        &prepared.fixture.core.memory,
        source.base(),
        &[1, 2, 3, 4, 5, 6, 7, 8],
    )
    .expect("second source bytes");
    test_support::submit(
        &mut prepared.fixture.core,
        prepared.queue,
        test_support::copy(
            99,
            source.base().raw(),
            prepared.destinations[1].base().raw(),
            4,
        ),
    );
    test_support::doorbell(&mut prepared.fixture.core, prepared.queue);
    prepared
}
