use super::{channel::Channel, events::Events, monitor::Monitor, resources::Resources};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_software::control::{InterruptEvent, ManagementAddresses};
use std::{convert::Infallible, future::Future};
mod request;

struct PendingRequest {
    response: ControlResponse,
    operation: super::contexts::Pending,
}

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
        let mut pending: Option<PendingRequest> = None;
        loop {
            self.observe((&monitor, Some(&mut events))).await?;
            if !channel.has_response_space().await? {
                self.interrupt.wait().await;
                continue;
            }
            if let Some(waiting) = pending.as_mut() {
                let completed = self
                    .watch(
                        self.poll_context(&resources, (&monitor, waiting)),
                        (&monitor, Some(&mut events)),
                    )
                    .await??;
                if completed {
                    let result = waiting.response;
                    pending = None;
                    self.watch(channel.respond(result), (&monitor, Some(&mut events)))
                        .await??;
                    tokio::task::yield_now().await;
                    continue;
                }
            }
            let Some(request) = self
                .watch(channel.try_receive(), (&monitor, Some(&mut events)))
                .await??
            else {
                if pending.is_some() {
                    tokio::task::yield_now().await;
                } else {
                    self.interrupt.wait().await;
                }
                continue;
            };
            let recovery = match request.opcode {
                ControlRequestOpcode::ResetChannel
                | ControlRequestOpcode::ResetDma
                | ControlRequestOpcode::ResetEvents => true,
                ControlRequestOpcode::AcquireChannel
                | ControlRequestOpcode::ReleaseChannel
                | ControlRequestOpcode::CreateContext
                | ControlRequestOpcode::DrainContext
                | ControlRequestOpcode::AbortContext
                | ControlRequestOpcode::DestroyContext => false,
            };
            if pending.is_some() && !recovery {
                let mut busy = response(request);
                busy.status = ControlResponseStatus::ContextBusy;
                self.watch(channel.respond(busy), (&monitor, Some(&mut events)))
                    .await??;
                tokio::task::yield_now().await;
                continue;
            }
            let (result, operation) = self
                .dispatch((&mut resources, request), (&monitor, &mut events))
                .await?;
            if let Some(operation) = operation {
                pending = Some(PendingRequest {
                    response: result,
                    operation,
                });
                tokio::task::yield_now().await;
                continue;
            }
            self.watch(channel.respond(result), (&monitor, Some(&mut events)))
                .await??;
            self.observe((&monitor, Some(&mut events))).await?;
            tokio::task::yield_now().await;
        }
    }

    async fn poll_context(
        &self,
        resources: &Resources,
        (monitor, waiting): (&Monitor<'_, B>, &mut PendingRequest),
    ) -> Result<bool, Error<B::Error, M::Error>> {
        let Some(contexts) = resources.contexts.as_ref() else {
            waiting.response.status = ControlResponseStatus::InvalidContext;
            return Ok(true);
        };
        match contexts
            .poll(
                (self.bus, monitor),
                (&waiting.operation, &mut waiting.response),
                resources.channels(waiting.operation.function),
            )
            .await
        {
            Err(Error::DeviceFailed) => {
                waiting.response.status = ControlResponseStatus::DeviceFailed;
                Ok(true)
            }
            Ok(completed) => {
                if completed && monitor.failed() {
                    waiting.response.status = ControlResponseStatus::DeviceFailed;
                }
                Ok(completed)
            }
            Err(error) => Err(error),
        }
    }

    async fn observe(
        &self,
        (monitor, events): (&Monitor<'_, B>, Option<&mut Events<'_, B, M>>),
    ) -> Result<(), Error<B::Error, M::Error>> {
        monitor.observe().await?;
        if let Some(events) = events
            && let Some(event) = monitor.take_event()
        {
            events.publish(event).await?;
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
