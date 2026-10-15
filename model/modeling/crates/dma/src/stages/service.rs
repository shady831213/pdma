mod dispatch;
use super::{Commands, Fresh, ReadPipeline, UpdateIrq, WriteCopy};
use crate::{
    ChunkInput, CommandRequest, Dma, Error, ExecutionContext, ExecutionError, MemoryFailure,
    ResetControl, Responses, Work,
};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::ChannelId;
type FreshState = hardware_interface_pipeline::ThenState<
    Option<Option<crate::CommandSnapshot>>,
    Option<Option<(Work, crate::channel::RangeCheck)>>,
>;
type AdmissionState = Option<
    hardware_interface_pipeline::FanState<
        (),
        Option<CommandRequest>,
        Option<Option<CommandRequest>>,
        Option<CommandRequest>,
    >,
>;

/// Finite admission or chunk execution with independent IRQ and reset hardware.
#[derive(Debug)]
pub struct Service<A = ReadPipeline, B = Responses> {
    pub(super) command: Commands,
    pub(super) execution: Then<A, Then<WriteCopy, B>>,
    pub(super) fresh: Fresh,
    pub(super) count: usize,
    pub(super) reset: ResetControl,
    pub(super) irq: UpdateIrq,
}

/// Invocation-local admission and execution paths; no state is stored in hardware.
#[derive(Debug)]
pub enum ServiceState<A, F, E, R, P> {
    /// Select an idle or occupied engine path.
    Select,
    /// Gather candidates without consuming a command.
    Admission(A),
    /// Fetch and validate one fresh command through ordinary Then dataflow.
    Fresh(ChannelId, F),
    /// Advance a finite typed chunk graph.
    Execution(ChannelId, E),
    /// Retry retained transformed bytes without reentering the read graph.
    RetryWrite(ChannelId, R),
    /// Publish a failed command directly through its configured completion tail.
    Completion(ChannelId, P),
}

impl<M: MemoryAccess<SB>, SB, A, B, T, R, P> Stage<Dma<M, SB>> for Service<A, B>
where
    M::Error: Into<MemoryFailure>,
    for<'a> Then<A, Then<WriteCopy, B>>: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Option<ChunkInput>,
            Output = (),
            Error = ExecutionError,
            State = T,
        >,
    for<'a> Then<WriteCopy, B>: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Option<crate::Payload>,
            Output = (),
            Error = ExecutionError,
            State = R,
        >,
    for<'a> B: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Option<crate::Completion>,
            Output = (),
            Error = ExecutionError,
            State = P,
        >,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = Option<ServiceState<AdmissionState, FreshState, T, R, P>>;
    fn start((): ()) -> Self::State {
        Some(ServiceState::Select)
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        let retained = state.as_mut().ok_or(RunError::Exhausted)?;
        device.retire_stopped_work();
        let result = if device.discard_aborted() {
            Ok(StepOutcome::Complete(Ok(())))
        } else {
            self.advance(device, retained)
        };
        device.retire_stopped_work();
        device.refresh_maintenance();
        let irq = self.irq.step(device, &mut ())?;
        let reset = self.reset.step(device, &mut ())?;
        let (StepOutcome::Complete(Ok(())), StepOutcome::Complete(Ok(()))) = (irq, reset) else {
            return Err(RunError::Exhausted);
        };
        match result {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(Err(ExecutionError::CommandFailed(completion)))) => {
                let id = completion.channel();
                *state = Some(ServiceState::Completion(id, B::start(Some(completion))));
                Ok(StepOutcome::InProgress)
            }
            Ok(StepOutcome::Complete(Ok(()))) => {
                *state = None;
                Ok(StepOutcome::Complete(Ok(())))
            }
            Ok(StepOutcome::Complete(Err(ExecutionError::Rejected(error)))) => {
                *state = None;
                Ok(StepOutcome::Complete(match error {
                    Error::Reset | Error::Halted | Error::ChannelFatal(_) => Ok(()),
                    error => Err(error),
                }))
            }
            Err(error) => {
                *state = None;
                Err(error)
            }
        }
    }
}
