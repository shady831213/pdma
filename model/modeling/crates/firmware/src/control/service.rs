use super::{channel::Channel, events::Events, monitor::Monitor, resources::Resources};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_software::control::{InterruptEvent, ManagementAddresses};
use std::{convert::Infallible, future::Future};
mod request;

pub(super) struct Service<'a, B, M> {
    pub(super) bus: &'a B,
    pub(super) memory: &'a M,
    pub(super) interrupt: &'a InterruptEvent,
    pub(super) addresses: ManagementAddresses,
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Sync> Service<'_, B, M> {
    pub(super) async fn run(
        self,
        mut resources: Resources,
    ) -> Result<Infallible, Error<B::Error, M::Error>> {
        let monitor = Monitor::new(self.bus, self.addresses, resources.channel_count()).await?;
        let mut channel = loop {
            monitor.observe().await?;
            if let Some(channel) = self
                .watch(
                    Channel::try_attach(self.bus, self.memory, self.addresses.control),
                    (&monitor, None),
                )
                .await??
            {
                break channel;
            }
            self.interrupt.wait().await;
        };
        let mut events = Events::attach(
            (self.bus, self.memory),
            (self.addresses.control, channel.descriptor()),
        )
        .await?;
        loop {
            self.observe((&monitor, Some(&mut events))).await?;
            if !channel.has_response_space().await? {
                self.interrupt.wait().await;
                continue;
            }
            let Some(request) = self
                .watch(channel.try_receive(), (&monitor, Some(&mut events)))
                .await??
            else {
                self.interrupt.wait().await;
                continue;
            };
            let result = match request.opcode {
                ControlRequestOpcode::ResetEvents => {
                    let mut result = response(request);
                    result.status = events.reset().await?;
                    result
                }
                ControlRequestOpcode::ResetDma
                | ControlRequestOpcode::AcquireChannel
                | ControlRequestOpcode::ReleaseChannel
                | ControlRequestOpcode::ResetChannel
                | ControlRequestOpcode::CreateContext
                | ControlRequestOpcode::DrainContext
                | ControlRequestOpcode::AbortContext
                | ControlRequestOpcode::DestroyContext => {
                    let result = self
                        .watch(
                            self.execute(&mut resources, (&monitor, request)),
                            (&monitor, Some(&mut events)),
                        )
                        .await?;
                    self.observe((&monitor, Some(&mut events))).await?;
                    match result {
                        Ok(mut response) => {
                            if monitor.failed() {
                                response.status = ControlResponseStatus::DeviceFailed;
                            }
                            response
                        }
                        Err(Error::DeviceFailed) => {
                            let mut failed = response(request);
                            failed.status = ControlResponseStatus::DeviceFailed;
                            failed
                        }
                        Err(error) => return Err(error),
                    }
                }
            };
            self.watch(channel.respond(result), (&monitor, Some(&mut events)))
                .await??;
            self.observe((&monitor, Some(&mut events))).await?;
            tokio::task::yield_now().await;
        }
    }

    async fn observe(
        &self,
        (monitor, events): (&Monitor<'_, B>, Option<&mut Events<'_, B, M>>),
    ) -> Result<(), Error<B::Error, M::Error>> {
        monitor.observe().await?;
        if let Some(events) = events {
            if let Some(event) = monitor.take_event() {
                events.publish(event).await?;
            }
        }
        Ok(())
    }

    async fn watch<F: Future>(
        &self,
        future: F,
        (monitor, mut events): (&Monitor<'_, B>, Option<&mut Events<'_, B, M>>),
    ) -> Result<F::Output, Error<B::Error, M::Error>> {
        let mut future = std::pin::pin!(future);
        loop {
            tokio::select! {
                biased;
                result = &mut future => return Ok(result),
                () = self.interrupt.wait() => { self.observe((monitor, events.as_deref_mut())).await?; }
            }
        }
    }
}

const fn response(request: ControlRequest) -> ControlResponse {
    ControlResponse {
        id: request.id,
        status: ControlResponseStatus::Success,
        channel: request.channel,
        context: request.context,
    }
}
