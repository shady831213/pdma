use super::{channel::Channel, resources::Resources};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_software::control::InterruptEvent;
use std::convert::Infallible;
mod request;

pub(super) struct Service<'a, B, M> {
    pub(super) bus: &'a B,
    pub(super) memory: &'a M,
    pub(super) interrupt: &'a InterruptEvent,
    pub(super) base: u32,
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Sync> Service<'_, B, M> {
    pub(super) async fn run(self, mut resources: Resources) -> Result<Infallible, Error<B::Error>> {
        let mut channel = loop {
            if let Some(channel) = Channel::try_attach(self.bus, self.memory, self.base).await? {
                break channel;
            }
            self.interrupt.wait().await;
        };
        loop {
            if !channel.has_response_space().await? {
                self.interrupt.wait().await;
                continue;
            }
            let Some(request) = channel.try_receive().await? else {
                self.interrupt.wait().await;
                continue;
            };
            let response = self.execute(&mut resources, request).await?;
            channel.respond(response).await?;
            tokio::task::yield_now().await;
        }
    }
}
