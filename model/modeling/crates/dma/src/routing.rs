use crate::{ChannelTask, Dma, Error, Publish, Resettable, UpdateIrq};
use hardware_interface_pipeline::{AfterEach, Route};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::Response;

/// A response destination bound to one channel and guarded by its reset generation.
pub type ResponsePort = Resettable<AfterEach<Publish, UpdateIrq>>;

/// Dispatches a completed command to its owning channel's publication port.
#[derive(Debug)]
pub struct RouteResponse;

impl<M: MemoryAccess> Route<Dma<M>, ResponsePort> for RouteResponse {
    type Input = ChannelTask<Response>;

    fn route(&mut self, _: &Dma<M>, input: Self::Input) -> Result<(usize, Self::Input), Error> {
        Ok((input.channel().index(), input))
    }
}
