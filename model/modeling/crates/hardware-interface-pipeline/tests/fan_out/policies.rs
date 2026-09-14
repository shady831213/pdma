use hardware_interface_pipeline::{BranchError, DeliveryPlan, Distribute};

use super::fixtures::{Context, Destination, Parcel, Receipt};

pub(super) struct Split;

impl Distribute<Context, Destination> for Split {
    type Input = Parcel;
    type Output = usize;
    type State = usize;

    fn split(
        &mut self,
        _: &Context,
        Parcel(mut input): Parcel,
    ) -> DeliveryPlan<usize, Parcel, &'static str> {
        let length = input.len();
        let tail = input.split_off(3);
        Ok((length, vec![(1, Parcel(input)), (0, Parcel(tail))]))
    }

    fn combine(
        &mut self,
        length: usize,
        outputs: Vec<Receipt>,
    ) -> Result<usize, BranchError<&'static str>> {
        Ok(length * outputs.len())
    }
}

pub(super) struct Broadcast(pub usize);

impl Distribute<Context, Destination> for Broadcast {
    type Input = String;
    type Output = Vec<Receipt>;
    type State = ();

    fn split(&mut self, _: &Context, input: String) -> DeliveryPlan<(), Parcel, &'static str> {
        Ok((
            (),
            (0..self.0)
                .map(|index| (index, Parcel(input.clone())))
                .collect(),
        ))
    }

    fn combine(
        &mut self,
        (): (),
        outputs: Vec<Receipt>,
    ) -> Result<Vec<Receipt>, BranchError<&'static str>> {
        Ok(outputs)
    }
}

pub(super) enum Reject {
    Split,
    Combine,
}

impl Distribute<Context, Destination> for Reject {
    type Input = Parcel;
    type Output = ();
    type State = ();

    fn split(&mut self, _: &Context, input: Parcel) -> DeliveryPlan<(), Parcel, &'static str> {
        match self {
            Self::Split => Err(BranchError::Application("split failed")),
            Self::Combine => Ok(((), vec![(0, input)])),
        }
    }

    fn combine(&mut self, (): (), _: Vec<Receipt>) -> Result<(), BranchError<&'static str>> {
        Err(BranchError::Application("combine failed"))
    }
}
