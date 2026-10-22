//! PF lifecycle management and VF data-path software.
use super::{Handler, HostVector, Ports, TestResult, buffers, transfer};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::firmware;
use pdma_model_software::control::InterruptEvent;
use std::time::Duration;
use tokio::sync::oneshot;
pub mod reset;

#[derive(Debug)]
/// Software-observed completion, copied bytes and ISR count.
pub struct Outcome {
    pub(crate) channel: ChannelId,
    pub(crate) completion: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) interrupts: usize,
}

pub(in crate::harness) struct Grant {
    pub(crate) channel: oneshot::Receiver<ChannelId>,
    pub(crate) finished: oneshot::Sender<()>,
}

pub(in crate::harness) struct Assignment {
    pub(crate) function: FunctionId,
    pub(crate) channel: oneshot::Sender<ChannelId>,
    pub(crate) finished: oneshot::Receiver<()>,
}

pub(in crate::harness) async fn manage(
    ports: Ports<'_>,
    assignments: [Assignment; 2],
    reset: Option<(oneshot::Receiver<()>, oneshot::Sender<()>)>,
) -> TestResult {
    firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        Duration::from_secs(1),
    )
    .await?;
    let interrupt = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(interrupt.handler()))?;
    let mut management = buffers::management(&ports, interrupt).await?;
    let mut channels = Vec::new();
    for assignment in assignments {
        let id = management.acquire_channel(assignment.function).await?;
        assignment
            .channel
            .send(id)
            .map_err(|_| "VF stopped before grant")?;
        channels.push((assignment.function, id, assignment.finished));
    }
    if let Some((requested, completed)) = reset {
        requested.await?;
        let (function, id, _) = channels.first().ok_or("missing reset target")?;
        management.reset_channel(*function, *id).await?;
        completed.send(()).map_err(|()| "VF stopped during reset")?;
    }
    for (function, id, finished) in channels {
        finished.await?;
        management.release_channel(function, id).await?;
    }
    Ok(())
}

pub(in crate::harness) async fn main(
    ports: Ports<'_>,
    payload: &[u8],
    grant: Grant,
) -> TestResult<Outcome> {
    let channel = grant.channel.await?;
    let memory = ports.memory.clone();
    let (completion, destination, deliveries) = transfer::run(ports, payload, channel).await?;
    let mut bytes = vec![0; payload.len()];
    memory.read(destination, MemoryAlignment::ONE, &mut bytes)?;
    grant
        .finished
        .send(())
        .map_err(|()| "PF stopped before release")?;
    Ok(Outcome {
        channel,
        completion,
        bytes,
        interrupts: deliveries.load(std::sync::atomic::Ordering::SeqCst),
    })
}
