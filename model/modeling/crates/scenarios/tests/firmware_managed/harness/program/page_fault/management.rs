use super::{Grant, Handler, HostVector, Ports, TestResult};
use crate::harness::{program::buffers, runtime::AccessError};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    control::ControlResponseStatus,
    dma::{ChannelRegisters, ContextId},
    memory::{ContextAttributes, FunctionId},
};
use pdma_model_host::firmware;
use pdma_model_software::control::InterruptEvent;
use std::time::Duration;
use tokio::sync::oneshot;

pub(in crate::harness) struct Lifecycle {
    pub grant: oneshot::Sender<Grant>,
    pub finished: oneshot::Receiver<()>,
    pub released: oneshot::Sender<()>,
}

pub(in crate::harness) async fn run<M: Send>(
    ports: Ports<'_, M>,
    lifecycle: Lifecycle,
    attributes: [ContextAttributes; 4],
) -> TestResult<Result<(), AccessError>> {
    firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        Duration::from_secs(1),
    )
    .await?;
    let control_irq = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(control_irq.handler()))?;
    let mut management =
        buffers::management_with_context(&ports, control_irq, attributes[1]).await?;
    let function = FunctionId::new(1);
    let mut contexts = Vec::new();
    for attributes in attributes {
        management
            .submit_create_context(function, attributes)
            .await?;
        let response = management.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err("context creation failed".into());
        }
        contexts.push(ContextId::new(response.context));
    }
    let contexts: [ContextId; 4] = contexts.try_into().map_err(|_| "context count")?;
    let channel = management.acquire_channel(function).await?;
    let resume = ports.bus.addresses().dma
        + u32::try_from(
            channel.register_offset(1).ok_or("channel address")?
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RESUME.offset(),
        )?;
    let foreign_resume = ports.bus.write(resume, &1_u32.to_le_bytes()).await;
    lifecycle
        .grant
        .send(Grant { channel, contexts })
        .map_err(|_| "VF stopped before grant")?;
    lifecycle.finished.await?;
    management.release_channel(function, channel).await?;
    for id in contexts {
        management.submit_destroy_context(function, id).await?;
        if management.next_response().await?.status != ControlResponseStatus::Success {
            return Err("context destroy failed".into());
        }
    }
    lifecycle
        .released
        .send(())
        .map_err(|()| "VF stopped before release")?;
    Ok(foreign_resume)
}
