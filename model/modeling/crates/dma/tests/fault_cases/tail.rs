use super::*;
use hardware_interface_pipeline::{HardwareDriver, Stage, StepResult, Then};
use pdma_model_dma::{Completion, ExecutionContext, PrepareChunk, ReadCopy, WriteCopy};
use std::{cell::Cell, rc::Rc};

struct ObserveCompletion(Rc<Cell<usize>>);

impl Stage<ExecutionContext<'_, Paged>> for ObserveCompletion {
    type Input = pdma_model_dma::Flow<Completion>;
    type Output = pdma_model_dma::Flow<Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, Paged>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(completion) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        if completion.data().is_some() {
            self.0.set(self.0.get() + 1);
        }
        Ok(StepOutcome::Complete(Ok(completion)))
    }
}

#[test]
fn resumed_write_uses_the_same_custom_completion_tail_as_healthy_write() -> TestResult {
    for fault in [false, true] {
        // Given: a real copy with an observable custom stage after WriteCopy.
        let mut device = setup()?.dma;
        device
            .memory
            .memory
            .write(0x1200.into(), MemoryAlignment::ONE, b"tail")?;
        submit(&mut device, crate::support::command(), 0)?;
        if fault {
            device.memory.missing.set(Some((0x1300, true)));
        }
        let count = Rc::new(Cell::new(0));
        let chunk = Then::new(PrepareChunk::default(), ReadCopy::default());
        let mut hardware = device.hardware_with(Then::new(
            pdma_model_dma::CommandExecution::new(Then::new(chunk, WriteCopy::default())),
            ObserveCompletion(Rc::clone(&count)),
        ));
        let mut driver = HardwareDriver::new();
        let mut repaired = false;
        // When: the failed write is repaired and resumes to successful publication.
        for _ in 0..256 {
            assert_eq!(
                driver.step(&mut hardware, &mut device)?,
                StepOutcome::InProgress
            );
            if fault_status(&mut device)? == 1 {
                assert!(!repaired);
                device.memory.missing.set(None);
                resume(&mut device, 1)?;
                repaired = true;
            }
            if count.get() == 1 {
                break;
            }
        }
        // Then: both ingress paths preserve the supplied completion tail exactly once.
        assert_eq!(repaired, fault);
        assert_eq!(
            read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
            0
        );
        assert_eq!(count.get(), 1, "fault={fault}");
        assert_eq!(
            device
                .memory
                .memory
                .read_buffer::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
            *b"tail"
        );
    }
    Ok(())
}
