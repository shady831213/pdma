use crate::{ChannelTask, Device, Error, Publish, Resettable, UpdateIrq};
use hardware_interface_pipeline::{AfterEach, BranchError, DeliveryPlan, Distribute};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::Response;

/// A response destination bound to one channel and guarded by its reset generation.
pub type ResponsePort = Resettable<AfterEach<Publish, UpdateIrq>>;

/// Dispatches a completed command to its owning channel's publication port.
#[derive(Debug)]
pub struct RouteResponse;

impl<M: MemoryAccess> Distribute<Device<M>, ResponsePort> for RouteResponse {
    type Input = ChannelTask<Response>;
    type Output = ();
    type State = ();

    fn split(&mut self, _: &Device<M>, input: Self::Input) -> DeliveryPlan<(), Self::Input, Error> {
        Ok(((), vec![(input.channel().index(), input)]))
    }

    fn combine(&mut self, (): (), outputs: Vec<ChannelTask<()>>) -> Result<(), BranchError<Error>> {
        if outputs.len() == 1 {
            Ok(())
        } else {
            Err(BranchError::OutputCount {
                expected: 1,
                actual: outputs.len(),
            })
        }
    }
}
