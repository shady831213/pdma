mod command;
mod engine;
mod intake;
mod port;

pub use command::{CommandArbiter, CommandArbiterState};
pub use intake::Intake;
pub use port::Candidate;

use crate::Error;
use hardware_interface_pipeline::{FanPlan, FanPolicy};
use pdma_model_contract::dma::ChannelId;

/// Selects a channel from completed candidate values in rotating order.
#[derive(Debug, Default)]
pub struct RoundRobin {
    next: usize,
}

impl FanPolicy<Option<ChannelId>, ChannelId, Error> for RoundRobin {
    fn plan(
        &mut self,
        values: &[Option<Option<ChannelId>>],
        available: &[bool],
    ) -> Result<FanPlan, Error> {
        if values.iter().any(Option::is_none) || !available.first().copied().unwrap_or(false) {
            return Ok(FanPlan {
                inputs: Vec::new(),
                outputs: Vec::new(),
            });
        }
        let selected = (0..values.len())
            .map(|offset| (self.next + offset) % values.len())
            .find(|index| {
                values
                    .get(*index)
                    .is_some_and(|value| value.as_ref().is_some_and(Option::is_some))
            });
        let mut inputs = Vec::with_capacity(values.len());
        if let Some(index) = selected {
            inputs.push(index);
            self.next = (index + 1) % values.len();
        }
        inputs.extend((0..values.len()).filter(|index| Some(*index) != selected));
        Ok(FanPlan {
            inputs,
            outputs: selected.map_or_else(Vec::new, |_| vec![0]),
        })
    }

    fn route(&mut self, selected: Vec<Option<ChannelId>>) -> Result<Vec<ChannelId>, Error> {
        Ok(selected.into_iter().next().flatten().into_iter().collect())
    }
}
