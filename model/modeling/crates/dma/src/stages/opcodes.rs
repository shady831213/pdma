use super::Fetch;
use super::execution::Install;
use super::operations::Execute;
use crate::{
    CheckRanges, ChunkInput, CommandRequest, Completion, CopyProgress, Dma, ExecutionContext,
    FetchedCommand, Flow,
};
use hardware_interface_pipeline::{Dispatch, FanOut, Route, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{
    AtomicCommand, Command, FenceCommand, FillCommand, SemaphoreCommand, TimestampCommand,
    TrapCommand, WaitCommand, WriteCommand,
};
use std::convert::Infallible;

type Copy<S> = Then<CheckRanges, Then<Install, S, Flow<ChunkInput>>, Flow<CopyProgress>>;

#[derive(Debug)]
pub(super) enum Branch<S> {
    Copy(Box<Copy<S>>),
    Write(Execute<WriteCommand>),
    Fill(Execute<FillCommand>),
    Wait(Execute<WaitCommand>),
    Atomic(Execute<AtomicCommand>),
    Semaphore(Execute<SemaphoreCommand>),
    Fence(Execute<FenceCommand>),
    Trap(Execute<TrapCommand>),
    Timestamp(Execute<TimestampCommand>),
    Empty,
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Dma<M, SB>> for Branch<S>
where
    M::Error: Into<crate::MemoryFailure>,
    for<'a> S: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Flow<ChunkInput>,
            Output = Flow<Completion>,
            Error = Infallible,
        >,
{
    type Input = Flow<FetchedCommand>;
    type Output = Flow<Completion>;
    type Error = Infallible;

    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let context = &mut dma.execution_context();
        match self {
            Self::Copy(copy) => copy.step(context, input),
            Self::Write(stage) => stage.step(context, input),
            Self::Fill(stage) => stage.step(context, input),
            Self::Wait(stage) => stage.step(context, input),
            Self::Atomic(stage) => stage.step(context, input),
            Self::Semaphore(stage) => stage.step(context, input),
            Self::Fence(stage) => stage.step(context, input),
            Self::Trap(stage) => stage.step(context, input),
            Self::Timestamp(stage) => stage.step(context, input),
            Self::Empty => Ok(input.take().map_or(StepOutcome::InProgress, |flow| {
                StepOutcome::Complete(Ok(flow.finish()))
            })),
        }
    }
}

#[derive(Debug)]
pub(super) struct Select;

impl Route<Flow<FetchedCommand>, Infallible> for Select {
    fn target(&mut self, input: &Flow<FetchedCommand>) -> Result<usize, Infallible> {
        Ok(match input.data().map(|fetched| &fetched.command) {
            Some(Command::Copy(_)) => 0,
            Some(Command::Write(_)) => 1,
            Some(Command::Fill(_)) => 2,
            Some(Command::Wait(_)) => 3,
            Some(Command::Atomic(_)) => 4,
            Some(Command::Semaphore(_)) => 5,
            Some(Command::Fence(_)) => 6,
            Some(Command::Trap(_)) => 7,
            Some(Command::Timestamp(_)) => 8,
            None => 9,
        })
    }
}

pub(super) type Branches<S> = FanOut<
    Fetch,
    Dispatch<Select>,
    Branch<S>,
    Option<CommandRequest>,
    Flow<FetchedCommand>,
    Flow<FetchedCommand>,
    Flow<Completion>,
    Infallible,
>;

pub(super) fn branches<S>(copy: S) -> Branches<S> {
    FanOut::new(
        Fetch,
        Dispatch::new(Select),
        vec![
            Branch::Copy(Box::new(Then::new(
                CheckRanges::default(),
                Then::new(Install, copy),
            ))),
            Branch::Write(Execute::default()),
            Branch::Fill(Execute::default()),
            Branch::Wait(Execute::default()),
            Branch::Atomic(Execute::default()),
            Branch::Semaphore(Execute::default()),
            Branch::Fence(Execute::default()),
            Branch::Trap(Execute::default()),
            Branch::Timestamp(Execute::default()),
            Branch::Empty,
        ],
    )
}
