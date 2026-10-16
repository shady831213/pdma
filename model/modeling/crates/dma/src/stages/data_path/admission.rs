use crate::{
    ChunkInput, CommandRequest, CommandSnapshot, CopyProgress, Dma, Error, ExecutionContext, Work,
};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};

#[derive(Debug)]
/// Selects current fresh or retained work while preserving an occupied grant.
pub struct Grant;

impl<M, SB> Stage<Dma<M, SB>> for Grant {
    type Input = Vec<(usize, Option<CommandRequest>)>;
    type Output = Option<(Work, Option<CommandSnapshot>)>;
    type Error = Error;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let requests = state.take().ok_or(RunError::Exhausted)?;
        if let Some(work) = dma.active {
            let channel = dma
                .channels
                .get_mut(work.channel.index())
                .ok_or(RunError::Exhausted)?;
            channel.resident = Some(work);
            return Ok(StepOutcome::Complete(Ok(Some((work, None)))));
        }
        let Some(request) = requests.into_iter().find_map(|(_, request)| request) else {
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        let request = match dma.command_request(request.channel()) {
            Ok(request) => request,
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
        };
        let output = match request {
            Some(CommandRequest::Fetch(snapshot)) => Some((snapshot.work, Some(snapshot))),
            Some(CommandRequest::Resume(work)) => {
                dma.active = Some(work);
                Some((work, None))
            }
            None => None,
        };
        if let Some((work, _)) = &output {
            let channel = dma
                .channels
                .get_mut(work.channel.index())
                .ok_or(RunError::Exhausted)?;
            channel.resident = Some(*work);
        }
        Ok(StepOutcome::Complete(Ok(output)))
    }
}

#[derive(Debug)]
pub struct Install;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Install {
    type Input = Option<CopyProgress>;
    type Output = Option<ChunkInput>;
    type Error = Error;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let result = match state.take().ok_or(RunError::Exhausted)? {
            Some(progress) => context.install_progress(progress).map(Some),
            None => Ok(context
                .active
                .filter(|work| *work == context.work())
                .map(ChunkInput::next)),
        };
        Ok(StepOutcome::Complete(result))
    }
}
