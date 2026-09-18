use super::{channel::Channel, resources::Resources};
use crate::{Configuration, Error};
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::{FunctionControlRegisters, FunctionId, RingError};
use pdma_model_software::control::InterruptEvent;
use std::convert::Infallible;
mod request;

pub(super) struct Service<'a, B, M> {
    pub(super) bus: &'a B,
    pub(super) memory: &'a M,
    pub(super) interrupt: &'a InterruptEvent,
    pub(super) config: Configuration,
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Sync> Service<'_, B, M> {
    pub(super) async fn run(self, mut resources: Resources) -> Result<Infallible, Error<B::Error>> {
        let layout = FunctionControlRegisters::new(self.config.function_count)
            .map_err(|_| Error::Ring(RingError::Geometry))?;
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(self.config.function_count)
            .map_err(Error::Allocation)?;
        channels.resize_with(self.config.function_count, || None);
        loop {
            let mut progressed = false;
            for (index, slot) in channels.iter_mut().enumerate() {
                let function = FunctionId::new(
                    u32::try_from(index).map_err(|_| Error::Ring(RingError::Geometry))?,
                );
                if slot.is_none() {
                    let offset = layout
                        .functions()
                        .offset(index)
                        .ok_or(Error::Ring(RingError::Geometry))?;
                    let base = self
                        .config
                        .addresses
                        .control
                        .checked_add(
                            u32::try_from(offset).map_err(|_| Error::Ring(RingError::Geometry))?,
                        )
                        .ok_or(Error::Ring(RingError::Geometry))?;
                    *slot = Channel::try_attach(self.bus, self.memory, base).await?;
                }
                let Some(channel) = slot else {
                    continue;
                };
                if !channel.has_response_space().await? {
                    continue;
                }
                let Some(request) = channel.try_receive().await? else {
                    continue;
                };
                let response = self.execute(&mut resources, (function, request)).await?;
                channel.respond(response).await?;
                progressed = true;
                tokio::task::yield_now().await;
            }
            if !progressed {
                self.interrupt.wait().await;
            }
        }
    }
}
