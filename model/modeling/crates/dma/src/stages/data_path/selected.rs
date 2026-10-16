use crate::{CommandRequest, CommandSnapshot, Dma, Error, ExecutionContext, Work};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};

#[derive(Debug)]
pub struct Selected<S> {
    child: S,
    work: Option<Work>,
    snapshot: Option<CommandSnapshot>,
    offered: bool,
}

impl<S> Selected<S> {
    pub const fn new(child: S) -> Self {
        Self {
            child,
            work: None,
            snapshot: None,
            offered: false,
        }
    }
}

impl<M, SB, S> Stage<Dma<M, SB>> for Selected<S>
where
    for<'a> S: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Option<CommandSnapshot>,
            Output = (),
            Error = Error,
        >,
{
    type Input = Option<CommandRequest>;
    type Output = ();
    type Error = Error;
    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<(), Error> {
        if self.work.is_none() {
            let Some(request) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            let Some(request) = request else {
                return Ok(StepOutcome::Complete(Ok(())));
            };
            let current = match dma.command_request(request.channel()) {
                Ok(current) => current,
                Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            };
            if current != Some(request) {
                return Ok(StepOutcome::Complete(Ok(())));
            }
            let (work, snapshot) = match request {
                CommandRequest::Fetch(snapshot) => (snapshot.work, Some(snapshot)),
                CommandRequest::Resume(task) => {
                    dma.active = Some(task.work);
                    (task.work, None)
                }
                CommandRequest::Continue(task) => (task.work, None),
            };
            let Some(channel) = dma.channels.get_mut(work.channel.index()) else {
                return Ok(StepOutcome::Complete(Err(Error::Unmapped)));
            };
            channel.resident = Some(work);
            self.work = Some(work);
            self.snapshot = snapshot;
            self.offered = true;
        }
        let Some(identity) = self.work else {
            return Ok(StepOutcome::InProgress);
        };
        let Ok(mut context) = dma.execution_context(identity.channel()) else {
            return Err(hardware_interface_pipeline::RunError::Exhausted);
        };
        context.generation = identity.generation;
        let mut input = self.offered.then(|| self.snapshot.take());
        let result = self.child.step(&mut context, &mut input);
        self.offered = input.is_some();
        self.snapshot = input.flatten();
        if matches!(result, Ok(StepOutcome::Complete(_))) {
            self.work = None;
            self.snapshot = None;
            self.offered = false;
            dma.release_invocation(identity);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CheckRanges, Fetch};
    use hardware_interface_pipeline::Operation;
    use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
    use pdma_model_contract::dma::{ChannelBlock, ChannelId, Command, CommandRegisters};
    use std::{cell::Cell, rc::Rc};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Observed(Rc<Cell<usize>>);
    impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Observed {
        type Input = Option<CommandSnapshot>;
        type Output = ();
        type Error = Error;
        fn step(
            &mut self,
            _: &mut ExecutionContext<'_, M, SB>,
            input: &mut Option<Self::Input>,
        ) -> StepResult<(), Error> {
            if input.take().is_none() {
                return Ok(StepOutcome::InProgress);
            }
            self.0.set(self.0.get() + 1);
            Ok(StepOutcome::Complete(Ok(())))
        }
    }

    #[test]
    fn reset_waits_for_resident_child_to_observe_and_complete_cleanup() -> TestResult {
        struct Resident {
            observations: Rc<Cell<usize>>,
            pending: Option<CommandSnapshot>,
        }
        impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Resident {
            type Input = Option<CommandSnapshot>;
            type Output = ();
            type Error = Error;
            fn step(
                &mut self,
                context: &mut ExecutionContext<'_, M, SB>,
                input: &mut Option<Self::Input>,
            ) -> StepResult<(), Error> {
                if self.pending.is_none() {
                    let Some(snapshot) = input.take() else {
                        return Ok(StepOutcome::InProgress);
                    };
                    let Some(snapshot) = snapshot else {
                        return Ok(StepOutcome::Complete(Ok(())));
                    };
                    self.pending = Some(snapshot);
                }
                if let Err(error) = context.lifecycle() {
                    self.observations.set(self.observations.get() + 1);
                    if self.observations.get() == 2 {
                        self.pending = None;
                        return Ok(StepOutcome::Complete(Err(error)));
                    }
                }
                Ok(StepOutcome::InProgress)
            }
        }
        // Given: the selected child owns unfinished local work.
        let mut dma = crate::stages::tests::queued_copy(0x1100, false)?;
        let observations = Rc::new(Cell::new(0));
        let mut selected = Selected::new(Resident {
            observations: Rc::clone(&observations),
            pending: None,
        });
        let mut input = Some(dma.command_request(ChannelId::new(0))?);
        assert_eq!(
            selected.step(&mut dma, &mut input)?,
            StepOutcome::InProgress
        );
        // When: reset becomes visible while the child needs two cleanup steps.
        dma.request_reset();
        assert_eq!(
            selected.step(&mut dma, &mut input)?,
            StepOutcome::InProgress
        );
        assert!(dma.resetting());
        assert!(selected.child.pending.is_some());
        let result = selected.step(&mut dma, &mut input)?;
        // Then: reset completes only after the child itself completes cleanup.
        assert_eq!(result, StepOutcome::Complete(Err(Error::Reset)));
        assert_eq!(observations.get(), 2);
        assert!(selected.child.pending.is_none());
        assert!(!dma.resetting());
        Ok(())
    }

    fn finish<C, S: Stage<C>>(
        stage: &mut S,
        context: &mut C,
        input: S::Input,
    ) -> TestResult<S::Output>
    where
        S::Error: std::error::Error + 'static,
    {
        let mut operation = Operation::new(input);
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(stage, context)? {
                return Ok(result?);
            }
        }
        Err("selected task watchdog".into())
    }

    #[test]
    fn stale_continuation_cannot_enter_the_next_task_on_the_same_generation() -> TestResult {
        // Given: an observation of the first accepted task before its retirement.
        let mut dma = crate::stages::tests::queued_copy(0x1100, false)?;
        let id = ChannelId::new(0);
        let Some(CommandRequest::Fetch(snapshot)) = dma.command_request(id)? else {
            return Err("fresh task".into());
        };
        let mut context = dma.execution_context(id)?;
        let fetched = finish(&mut Fetch, &mut context, Some(snapshot))?.ok_or("fetched")?;
        let command = *fetched.command();
        let progress =
            finish(&mut CheckRanges::default(), &mut context, Some(fetched))?.ok_or("validated")?;
        let _ = context.install_progress(progress)?;
        let old = dma.command_request(id)?;
        let mut hardware = dma.hardware();
        finish(&mut hardware, &mut dma, ())?;
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
        let Some(CommandRequest::Fetch(snapshot)) = dma.command_request(id)? else {
            return Err("next task".into());
        };
        let _ = finish(&mut Fetch, &mut dma.execution_context(id)?, Some(snapshot))?;
        let current = dma.command_request(id)?;
        let calls = Rc::new(Cell::new(0));
        let mut selected = Selected::new(Observed(Rc::clone(&calls)));
        // When: the old continuation reaches the receiving boundary after next-task acceptance.
        finish(&mut selected, &mut dma, old)?;
        // Then: it cannot enter the new task, while that task's own observation can.
        assert_eq!(calls.get(), 0);
        finish(&mut selected, &mut dma, current)?;
        assert_eq!(calls.get(), 1);
        Ok(())
    }
}
