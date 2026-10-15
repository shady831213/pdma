use super::*;
use hardware_interface_pipeline::{HardwareDriver, RunError, Stage, StepResult, Then};
use pdma_model_dma::{
    Completion, ExecutionContext, ExecutionError, PrepareChunk, ReadCopy, WriteCopy,
};
use std::{cell::Cell, rc::Rc};

struct ObserveCompletion(Rc<Cell<usize>>);

impl Stage<ExecutionContext<'_, Paged>> for ObserveCompletion {
    type Input = Option<Completion>;
    type Output = Option<Completion>;
    type Error = ExecutionError;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, Paged>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, ExecutionError> {
        let completion = state.take().ok_or(RunError::Exhausted)?;
        if completion.is_some() {
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
        let chunk = Then::new(PrepareChunk, ReadCopy);
        let mut hardware = device.service_with(Then::new(
            chunk,
            Then::new(
                WriteCopy,
                Then::new(ObserveCompletion(Rc::clone(&count)), device.responses()),
            ),
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
            if read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())? == 1 {
                break;
            }
        }
        // Then: both ingress paths preserve the supplied completion tail exactly once.
        assert_eq!(repaired, fault);
        assert_eq!(
            read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
            1
        );
        assert_eq!(count.get(), 1, "fault={fault}");
        assert_eq!(
            device
                .memory
                .memory
                .read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
            *b"tail"
        );
    }
    Ok(())
}
