use crate::{Dma, Error};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use pdma_model_contract::dma::FaultStatusValues;

/// Multiplexes one composed engine, retaining its exact operation only on payload suspension.
#[derive(Debug)]
pub struct Schedule<S> {
    engine: S,
}

impl<S> Schedule<S> {
    /// Wraps a repeated command engine without changing its child-stage continuation.
    pub const fn new(engine: S) -> Self {
        Self { engine }
    }
}

/// One runnable operation and at most one parked continuation per physical channel.
pub struct ScheduleState<S: Stage<Dma<M, SB>>, M, SB> {
    active: Operation<S, Dma<M, SB>>,
    parked: Vec<Option<Operation<S, Dma<M, SB>>>>,
}

impl<S: Stage<Dma<M, SB>>, M, SB> std::fmt::Debug for ScheduleState<S, M, SB> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScheduleState")
            .field("active_complete", &self.active.is_complete())
            .field(
                "parked_count",
                &self.parked.iter().filter(|slot| slot.is_some()).count(),
            )
            .finish_non_exhaustive()
    }
}

impl<M, SB, S> Stage<Dma<M, SB>> for Schedule<S>
where
    S: Stage<Dma<M, SB>, Input = (), Output = (), Error = Error, Wait = ()>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type Wait = ();
    type State = ScheduleState<S, M, SB>;

    fn start((): ()) -> Self::State {
        ScheduleState {
            active: Operation::new(()),
            parked: Vec::new(),
        }
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepOutcome<(), Error, ()> {
        if state.parked.is_empty() {
            if state
                .parked
                .try_reserve_exact(device.channels.len())
                .is_err()
            {
                return StepOutcome::Complete(Err(Error::SchedulerAllocation));
            }
            state.parked.resize_with(device.channels.len(), || None);
        }
        for (index, (channel, parked)) in device
            .channels
            .iter_mut()
            .zip(&mut state.parked)
            .enumerate()
        {
            let aborts = device.maintenance.as_ref().is_some_and(|maintenance| {
                maintenance.abort
                    && maintenance.result == Ok(false)
                    && maintenance
                        .frontiers
                        .iter()
                        .any(|frontier| frontier.channel.index() == index)
                    && channel
                        .parked
                        .as_ref()
                        .is_some_and(|task| task.value.contains(&Some(maintenance.context)))
            });
            if channel.resetting() || aborts {
                *parked = None;
                channel.parked = None;
                channel.registers.values.fault_status = FaultStatusValues::new(0);
            }
        }
        let outcome = state.active.step(&mut self.engine, device);
        if let Some(id) = device.resume_selected.take() {
            let Some(parked) = state.parked.get_mut(id.index()).and_then(Option::take) else {
                return StepOutcome::Complete(Err(Error::Routing));
            };
            state.active = parked;
            device.active = device
                .channels
                .get_mut(id.index())
                .and_then(|channel| channel.parked.take());
            return StepOutcome::Progressed;
        }
        if let Some(task) = &device.active {
            let Some(channel) = device.channels.get_mut(task.channel().index()) else {
                return StepOutcome::Complete(Err(Error::Routing));
            };
            if channel.registers.values.fault_status.faulted() {
                let Some(slot) = state.parked.get_mut(task.channel().index()) else {
                    return StepOutcome::Complete(Err(Error::Routing));
                };
                *slot = Some(std::mem::replace(&mut state.active, Operation::new(())));
                channel.parked = device.active.take();
                device.refresh_maintenance();
                return StepOutcome::Progressed;
            }
        }
        device.refresh_maintenance();
        outcome
    }
}
