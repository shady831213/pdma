//! Host software for one Function, using unchanged management and DMA drivers.
use super::{Handler, HostVector, Ports, TestResult, buffers, transfer};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{ChannelId, Response};
use pdma_model_software::control::InterruptEvent;
pub mod reset;

#[derive(Debug)]
/// Software-observed completion, copied bytes and ISR count.
pub struct Outcome {
    pub(crate) channel: ChannelId,
    pub(crate) response: Response,
    pub(crate) bytes: Vec<u8>,
    pub(crate) interrupts: usize,
}

pub(in crate::harness) async fn main(ports: Ports<'_>, payload: &[u8]) -> TestResult<Outcome> {
    let interrupt = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(interrupt.handler()))?;
    let mut management = buffers::management(&ports, interrupt).await?;
    let channel = management.acquire_channel().await?;
    let memory = ports.memory.clone();
    let (response, destination, deliveries) = transfer::run(ports, payload, channel).await?;
    let mut bytes = vec![0; payload.len()];
    memory.read(destination, MemoryAlignment::ONE, &mut bytes)?;
    management.reset_channel(channel).await?;
    management.release_channel(channel).await?;
    Ok(Outcome {
        channel,
        response,
        bytes,
        interrupts: deliveries.load(std::sync::atomic::Ordering::SeqCst),
    })
}
