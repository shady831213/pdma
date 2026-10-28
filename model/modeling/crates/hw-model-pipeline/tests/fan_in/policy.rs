use hw_model_pipeline::{FanPlan, FanPolicy};
pub(super) enum Policy {
    RoundRobin(usize),
    Fixed,
    Invalid(usize),
    Reject,
}
impl FanPolicy<Option<usize>, usize, &'static str> for Policy {
    fn plan(
        &mut self,
        values: &[Option<Option<usize>>],
        available: &[bool],
    ) -> Result<FanPlan, &'static str> {
        if available.first() != Some(&true) {
            return Ok(FanPlan::default());
        }
        let next = match self {
            Self::RoundRobin(next) => *next,
            Self::Fixed => 0,
            Self::Invalid(index) => {
                return Ok(FanPlan {
                    inputs: vec![*index],
                    outputs: vec![0],
                });
            }
            Self::Reject => return Err("selection failed"),
        };
        let selected = (next..values.len())
            .chain(0..next.min(values.len()))
            .find(|&index| matches!(values.get(index), Some(Some(Some(_)))));
        let mut inputs: Vec<_> = selected.into_iter().collect();
        inputs.extend(
            values
                .iter()
                .enumerate()
                .filter_map(|(i, v)| (v.is_some() && Some(i) != selected).then_some(i)),
        );
        if let (Self::RoundRobin(next), Some(index)) = (self, selected) {
            *next = (index + 1) % values.len();
        }
        Ok(FanPlan {
            inputs,
            outputs: selected.map_or_else(Vec::new, |_| vec![0]),
        })
    }
    fn route(&mut self, values: Vec<Option<usize>>) -> Result<Vec<usize>, &'static str> {
        Ok(values.into_iter().next().flatten().into_iter().collect())
    }
}
