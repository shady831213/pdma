//! Finite service boundaries retain hardware traversal and accepted ownership.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelBlock, CommandRegisters, Endpoint, ResponseRegisters};
use pdma_model_dma::{COPY_CHUNK_BYTES, ResetControl};
use std::{cell::Cell, rc::Rc};
use support::{TestResult, command, read_word, setup, submit};

struct Witness(Rc<Cell<usize>>);

impl Drop for Witness {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Tail {
    entered: Rc<Cell<bool>>,
    release: Rc<Cell<bool>>,
    drops: Rc<Cell<usize>>,
}

impl<M> Stage<pdma_model_dma::Dma<M>> for Tail {
    type Input = ();
    type Output = ();
    type Error = pdma_model_dma::Error;
    type State = Option<Witness>;

    fn start((): ()) -> Self::State {
        None
    }

    fn step(
        &mut self,
        _: &mut pdma_model_dma::Dma<M>,
        state: &mut Self::State,
    ) -> hardware_interface_pipeline::StepResult<(), Self::Error> {
        state.get_or_insert_with(|| Witness(Rc::clone(&self.drops)));
        self.entered.set(true);
        Ok(if self.release.get() {
            StepOutcome::Complete(Ok(()))
        } else {
            StepOutcome::InProgress
        })
    }
}

const CONSUMER: usize =
    ChannelBlock::ALIGNMENT + ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset();
const PRODUCER: usize = ChannelBlock::ALIGNMENT
    + ChannelBlock::RESPONSE.offset()
    + ResponseRegisters::PRODUCER.offset();

#[test]
fn finite_service_preserves_progress_without_fetching_again() -> TestResult {
    // Given: a descriptor requiring three bounded copy interactions.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    descriptor.copy_length = 600;
    descriptor.source = Endpoint::linear(0x1400, 0);
    descriptor.destination = Endpoint::linear(0x1800, 0);
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; 600])?;
    submit(&mut device, &memory, descriptor)?;
    let mut service = device.service();
    // When: each external invocation completes before the next chunk begins.
    let total = usize::try_from(descriptor.copy_length)?;
    for expected in [COPY_CHUNK_BYTES, COPY_CHUNK_BYTES * 2, total] {
        let mut operation = Operation::new(());
        let mut finished = false;
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
                result?;
                finished = true;
                break;
            }
        }
        // Then: one chunk advances and the descriptor is accepted exactly once.
        assert!(
            finished,
            "a service invocation must complete after one chunk"
        );
        let mut bytes = [0; 600];
        memory.read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
        let mut expected_bytes = [0; 600];
        expected_bytes
            .get_mut(..expected)
            .ok_or("prefix")?
            .fill(0x5a);
        assert_eq!(bytes, expected_bytes);
        assert_eq!(read_word(&mut device, CONSUMER)?, 1);
        assert_eq!(
            read_word(&mut device, PRODUCER)?,
            u32::from(expected == total)
        );
        memory.write(0x1000.into(), MemoryAlignment::ONE, &command().to_bytes()?)?;
    }
    Ok(())
}

#[test]
fn reset_between_service_invocations_retires_retained_progress() -> TestResult {
    // Given: accepted command state retained at a finite service boundary.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    descriptor.copy_length = 600;
    descriptor.source = Endpoint::linear(0x1400, 0);
    descriptor.destination = Endpoint::linear(0x1800, 0);
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; 600])?;
    submit(&mut device, &memory, descriptor)?;
    let mut service = device.service();
    let mut operation = Operation::new(());
    let mut finished = false;
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
            result?;
            finished = true;
            break;
        }
    }
    assert!(finished);
    // When: reset arrives while no service invocation owns the traversal.
    let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
    device.write(reset, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, reset)?, 1);
    let mut cleanup = Operation::new(());
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = cleanup.step(&mut service, &mut device)? {
            result?;
            break;
        }
    }
    // Then: cleanup acknowledges reset without touching the remaining destination.
    assert_eq!(read_word(&mut device, reset)?, 0);
    let mut bytes = [0; 600];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
    let mut expected_bytes = [0; 600];
    expected_bytes
        .get_mut(..COPY_CHUNK_BYTES)
        .ok_or("prefix")?
        .fill(0x5a);
    assert_eq!(bytes, expected_bytes);
    assert_eq!(read_word(&mut device, PRODUCER)?, 0);
    Ok(())
}

#[test]
fn reset_after_chunk_feedback_waits_for_composed_tail_to_drop() -> TestResult {
    // Given: a non-final chunk published progress, but the custom tail retains resources.
    let (mut device, memory) = setup(0x1100, 2)?;
    let descriptor = pdma_model_contract::dma::Command {
        copy_length: u64::try_from(COPY_CHUNK_BYTES * 2)?,
        source: Endpoint::linear(0x1400, 0),
        destination: Endpoint::linear(0x1800, 0),
        ..command()
    };
    submit(&mut device, &memory, descriptor)?;
    let entered = Rc::new(Cell::new(false));
    let release = Rc::new(Cell::new(false));
    let drops = Rc::new(Cell::new(0));
    let pipeline = hardware_interface_pipeline::Then::new(
        support::single_chunk(&device).downstream,
        Tail {
            entered: Rc::clone(&entered),
            release: Rc::clone(&release),
            drops: Rc::clone(&drops),
        },
    );
    let mut service = device.service_with(pipeline);
    let mut operation = Operation::new(());
    for _ in 0..256 {
        let _ = operation.step(&mut service, &mut device)?;
        if entered.get() {
            break;
        }
    }
    assert!(entered.get());
    // When: reset arrives after feedback but before the tail's terminal step.
    let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
    device.write(reset, &1_u32.to_le_bytes())?;
    for _ in 0..12 {
        assert!(matches!(
            operation.step(&mut service, &mut device)?,
            StepOutcome::InProgress
        ));
        assert_eq!(drops.get(), 0);
        assert_eq!(read_word(&mut device, reset)?, 1);
    }
    release.set(true);
    for _ in 0..32 {
        if let StepOutcome::Complete(result) = operation.step(&mut service, &mut device)? {
            result?;
            break;
        }
    }
    // Then: reset completes only after the composed continuation drops.
    assert_eq!(drops.get(), 1);
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert_eq!(read_word(&mut device, PRODUCER)?, 0);
    Ok(())
}

#[test]
fn reset_at_every_continuing_service_boundary_retires_without_more_writes() -> TestResult {
    for prefix in 0..24 {
        // Given: one committed chunk and each boundary of the next finite service.
        let (mut device, memory) = setup(0x1100, 2)?;
        let descriptor = pdma_model_contract::dma::Command {
            copy_length: u64::try_from(COPY_CHUNK_BYTES * 3)?,
            source: Endpoint::linear(0x1400, 0),
            destination: Endpoint::linear(0x1800, 0),
            ..command()
        };
        memory.write(
            0x1400.into(),
            MemoryAlignment::ONE,
            &[0x5a; COPY_CHUNK_BYTES * 3],
        )?;
        submit(&mut device, &memory, descriptor)?;
        let mut service = device.service();
        let mut first = Operation::new(());
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = first.step(&mut service, &mut device)? {
                result?;
                break;
            }
        }
        assert!(first.is_complete());
        let mut next = Operation::new(());
        for _ in 0..prefix {
            if let StepOutcome::Complete(result) = next.step(&mut service, &mut device)? {
                result?;
                break;
            }
        }
        let mut before = [0; COPY_CHUNK_BYTES * 3];
        memory.read(0x1800.into(), MemoryAlignment::ONE, &mut before)?;
        // When: reset arrives before or after the traversal transfers to the next child.
        let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
            + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
        device.write(reset, &1_u32.to_le_bytes())?;
        if next.is_complete() {
            next = Operation::new(());
        }
        for _ in 0..256 {
            if let StepOutcome::Complete(result) = next.step(&mut service, &mut device)? {
                result?;
                break;
            }
        }
        // Then: reset retires the accepted command and no old payload access occurs.
        assert!(next.is_complete(), "prefix {prefix} stalled");
        assert_eq!(read_word(&mut device, reset)?, 0, "prefix {prefix}");
        let mut after = [0; COPY_CHUNK_BYTES * 3];
        memory.read(0x1800.into(), MemoryAlignment::ONE, &mut after)?;
        assert_eq!(after, before, "prefix {prefix}");
    }
    Ok(())
}
