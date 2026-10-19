use crate::{ChunkInput, CommandRequest, Dma, Error, ExecutionContext, Fetch, Flow, Retire};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, Command, CommandRegisters};
use std::{cell::Cell, convert::Infallible, rc::Rc};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn finish<C, S: Stage<C>>(stage: &mut S, context: &mut C, input: S::Input) -> TestResult<S::Output>
where
    S::Error: std::error::Error + 'static,
{
    let mut operation = Operation::new(input);
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(stage, context)? {
            return Ok(result?);
        }
    }
    Err("task watchdog".into())
}

#[test]
fn reset_waits_for_resident_child_to_observe_and_complete_cleanup() -> TestResult {
    struct Resident {
        observations: Rc<Cell<usize>>,
        entered: Rc<Cell<bool>>,
        pending: Option<Flow<ChunkInput>>,
    }
    impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Resident {
        type Input = Flow<ChunkInput>;
        type Output = Flow<()>;
        type Error = Infallible;
        fn step(
            &mut self,
            context: &mut ExecutionContext<'_, M, SB>,
            input: &mut Option<Self::Input>,
        ) -> StepResult<Self::Output, Infallible> {
            if self.pending.is_none() {
                self.pending = input.take();
            }
            let Some(pending) = self.pending.as_ref() else {
                return Ok(StepOutcome::InProgress);
            };
            self.entered.set(true);
            if let Err(error) = pending.lifecycle(context) {
                self.observations.set(self.observations.get() + 1);
                if self.observations.get() == 2 {
                    let pending = self
                        .pending
                        .take()
                        .ok_or(hardware_interface_pipeline::RunError::Exhausted)?;
                    return Ok(StepOutcome::Complete(Ok(pending.stop(error))));
                }
            }
            Ok(StepOutcome::InProgress)
        }
    }
    // Given: the execution graph owns unfinished local work.
    let mut dma = crate::stages::tests::queued_copy(0x1100, false)?;
    let observations = Rc::new(Cell::new(0));
    let entered = Rc::new(Cell::new(false));
    let mut hardware = dma.hardware_with(Resident {
        observations: Rc::clone(&observations),
        entered: Rc::clone(&entered),
        pending: None,
    });
    let mut operation = Operation::new(());
    for _ in 0..128 {
        let _ = operation.step(&mut hardware, &mut dma)?;
        if entered.get() {
            break;
        }
    }
    assert!(entered.get());
    // When: reset becomes visible while the stage needs two cleanup steps.
    dma.request_reset();
    for _ in 0..128 {
        let _ = operation.step(&mut hardware, &mut dma)?;
        if observations.get() == 1 {
            break;
        }
    }
    assert_eq!(observations.get(), 1);
    assert!(dma.resetting());
    let mut completed = false;
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
            result?;
            completed = true;
            break;
        }
    }
    // Then: retirement acknowledges reset only after local cleanup completes.
    assert!(completed);
    assert_eq!(observations.get(), 2);
    assert!(!dma.resetting());
    Ok(())
}

#[test]
fn stale_fetch_cannot_enter_the_next_task_on_the_same_generation() -> TestResult {
    // Given: an observation of the first accepted task before retirement.
    let mut dma = crate::stages::tests::queued_copy(0x1100, false)?;
    let id = ChannelId::new(0);
    let request = dma.command_request(id)?;
    let old = request;
    let fetched = finish(&mut Fetch, &mut dma, request)?;
    let command = *fetched.data().ok_or("fetched")?.command();
    let progress = finish(
        &mut crate::CheckRanges::default(),
        &mut dma.execution_context(),
        fetched,
    )?;
    let chunk = dma.execution_context().install_progress(progress);
    let mut execution = crate::CommandExecution::new(hardware_interface_pipeline::Then::new(
        crate::PrepareChunk::default(),
        hardware_interface_pipeline::Then::new(
            crate::ReadCopy::default(),
            crate::WriteCopy::default(),
        ),
    ));
    let completion = finish(&mut execution, &mut dma.execution_context(), chunk)?;
    let response = finish(
        &mut dma.responses(),
        &mut dma.execution_context(),
        completion,
    )?;
    finish(&mut Retire, &mut dma, response)?;
    dma.memory.write(
        (0x1000 + u64::try_from(Command::RECORD_SIZE)?).into(),
        MemoryAlignment::ONE,
        &command.to_bytes()?,
    )?;
    dma.write(
        ChannelBlock::ALIGNMENT
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let next = dma.command_request(id)?;
    assert!(matches!(next, Some(CommandRequest::Fetch(_))));
    let fetched = finish(&mut Fetch, &mut dma, next)?;
    // When: a stale request reaches admission after next-task acceptance.
    let rejected = finish(&mut Fetch, &mut dma, old)?;
    // Then: rejection leaves the next task's resident ownership intact until cleanup.
    assert!(rejected.work.is_none());
    assert!(fetched.work.is_some());
    dma.request_reset();
    finish(&mut Retire, &mut dma, rejected.finish())?;
    assert!(dma.resetting());
    assert!(finish(&mut Retire, &mut dma, fetched.stop(Error::Reset)).is_err());
    assert!(!dma.resetting());
    Ok(())
}

#[test]
fn admission_does_not_replace_resident_ownership() -> TestResult {
    let mut dma: Dma<_> = crate::stages::tests::queued_copy(0x1100, false)?;
    let request = dma.command_request(ChannelId::new(0))?;
    let accepted = finish(&mut Fetch, &mut dma, request)?;
    let next = dma.command_request(ChannelId::new(0))?;
    let rejected = finish(&mut Fetch, &mut dma, next)?;
    assert!(rejected.work.is_none());
    dma.request_reset();
    finish(&mut Retire, &mut dma, rejected.finish())?;
    assert!(dma.resetting());
    let result = finish(&mut Retire, &mut dma, accepted.stop(Error::Reset));
    assert!(result.is_err());
    assert!(!dma.resetting());
    Ok(())
}

#[test]
fn custom_domain_stop_passes_through_retirement() -> TestResult {
    struct Stop;
    impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Stop {
        type Input = Flow<ChunkInput>;
        type Output = Flow<()>;
        type Error = Infallible;
        fn step(
            &mut self,
            _: &mut ExecutionContext<'_, M, SB>,
            input: &mut Option<Self::Input>,
        ) -> StepResult<Self::Output, Infallible> {
            Ok(input.take().map_or(StepOutcome::InProgress, |flow| {
                StepOutcome::Complete(Ok(flow.stop(Error::Routing)))
            }))
        }
    }
    let mut dma = crate::stages::tests::queued_copy(0x1100, false)?;
    let mut hardware = dma.hardware_with(Stop);
    let mut operation = Operation::new(());
    let mut outcome = None;
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
            outcome = Some(result);
            break;
        }
    }
    assert_eq!(outcome, Some(Err(Error::Routing)));
    assert!(
        dma.channels
            .iter()
            .all(|channel| channel.resident.is_none())
    );
    dma.request_reset();
    assert!(!dma.resetting());
    Ok(())
}

#[test]
fn custom_tail_preserves_first_domain_stop() -> TestResult {
    let mut dma = crate::stages::tests::queued_copy(0x1100, false)?;
    let request = dma.command_request(ChannelId::new(0))?;
    let accepted = finish(&mut Fetch, &mut dma, request)?;
    let stopped: Flow<()> = accepted
        .stop::<ChunkInput>(Error::Reset)
        .stop(Error::Routing);
    assert_eq!(stopped.status(), Err(Error::Reset));
    let mut input = Some(stopped);
    assert_eq!(
        Retire.step(&mut dma, &mut input)?,
        StepOutcome::Complete(Err(Error::Reset))
    );
    assert!(input.is_none());
    assert!(
        dma.channels
            .iter()
            .all(|channel| channel.resident.is_none())
    );
    Ok(())
}
