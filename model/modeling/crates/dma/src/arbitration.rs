use crate::{CommandRequest, Error};
use hardware_interface_pipeline::Arbitration;

/// Selects a fresh or resumed task with rotating channel priority.
#[derive(Debug, Default)]
pub struct RoundRobin {
    next: usize,
}

impl Arbitration<Option<CommandRequest>, Error> for RoundRobin {
    fn select(
        &mut self,
        values: &[Option<Option<CommandRequest>>],
    ) -> Result<Option<usize>, Error> {
        let selected = (0..values.len())
            .map(|offset| (self.next + offset) % values.len())
            .find(|index| {
                values
                    .get(*index)
                    .is_some_and(|value| value.as_ref().is_some_and(Option::is_some))
            });
        if let Some(index) = selected {
            self.next = (index + 1) % values.len();
        }
        Ok(selected)
    }
}
