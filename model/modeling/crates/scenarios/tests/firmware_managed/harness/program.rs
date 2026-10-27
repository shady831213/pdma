//! The complete Host program, including preboot access probes.

mod buffers;
#[doc = "Context lifecycle software program."]
pub mod contexts;
pub mod functions;
mod interrupts;
pub mod multi;
/// VF-owned page repair and retained DMA task resume scenario.
pub mod page_fault;
pub mod transfer;

use super::runtime::{AccessError, HostBus, HostIrq, HostVector};
use hardware_interface_io::memory::MemoryAccessError;
use hardware_interface_io::memory::SharedMemoryAllocator;
use hardware_interface_io::memory::{MemoryAddress, SharedMemoryAccess};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_software::InterruptRegistration;
use interrupts::Handler;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::firmware;
use pdma_model_software::control::InterruptEvent;
use std::time::Duration;

use super::TestResult;

/// The case-selected Host operation after firmware readiness.
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    /// Copy these bytes and wait for the interrupt handler's response.
    Copy(&'a [u8]),
    /// Exit with an intentional error before allocating application memory.
    StopAfterReady,
}

pub(super) struct Ports<'a, M = SharedMemoryAccess<(), MemoryAccessError>> {
    pub(super) bus: HostBus<M>,
    pub(super) memory: SharedMemoryAccess<(), MemoryAccessError>,
    pub(super) allocator: SharedMemoryAllocator,
    pub(super) interrupts: InterruptRegistration<'a, HostIrq, Handler>,
}

/// Host observations collected before polling firmware execution.
#[derive(Debug)]
pub struct Before {
    pub(crate) read: Result<(), AccessError>,
    pub(crate) write: Result<(), AccessError>,
    pub(crate) control_read: Result<(), AccessError>,
    pub(crate) control_write: Result<(), AccessError>,
    pub(crate) output: [u8; 4],
    pub(crate) ready: u32,
}

/// Completed transfer and permission observations for case-owned assertions.
#[derive(Debug)]
pub struct Outcome {
    pub(crate) before: Before,
    pub(crate) after: [Result<(), AccessError>; 3],
    pub(crate) completion: u64,
    pub(crate) destination: MemoryAddress,
    pub(crate) released: u32,
    pub(crate) dma_interrupts: usize,
}

pub(super) async fn main(ports: Ports<'_>, input: Input<'_>) -> TestResult<Outcome> {
    let addresses = ports.bus.addresses();
    let first = pdma_model_contract::dma::ChannelId::new(0);
    let window_control = addresses.window_control(first).ok_or("window address")?;
    let window_status = addresses.window_status(first).ok_or("window address")?;
    let channel_control = channel_address(&ports.bus, first).await?
        + u32::try_from(pdma_model_contract::dma::ChannelBlock::CONTROL.offset())?;
    let mut output = [0xa5; 4];
    let mut ready = [0; 4];
    let read = ports.bus.read(channel_control, &mut output).await;
    let write = ports.bus.write(channel_control, &1_u32.to_le_bytes()).await;
    let control_read = ports.bus.read(window_control, &mut [0; 4]).await;
    let control_write = ports.bus.write(window_control, &1_u32.to_le_bytes()).await;
    ports.bus.read(window_status, &mut ready).await?;
    let before = Before {
        read,
        write,
        control_read,
        control_write,
        output,
        ready: u32::from_le_bytes(ready),
    };
    firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        Duration::from_secs(1),
    )
    .await?;
    ports.bus.read(window_status, &mut ready).await?;
    if u32::from_le_bytes(ready) != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "firmware readiness unexpectedly granted DMA access",
        )
        .into());
    }
    let payload = match input {
        Input::Copy(payload) => payload,
        Input::StopAfterReady => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Host stopped after firmware readiness",
            )
            .into());
        }
    };
    let interrupt = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(interrupt.handler()))?;
    let mut management = buffers::management(&ports, interrupt).await?;
    let channel = management.acquire_channel(FunctionId::new(0)).await?;
    let after = [
        ports.bus.write(window_control, &0_u32.to_le_bytes()).await,
        ports.bus.write(window_control, &1_u32.to_le_bytes()).await,
        ports.bus.write(window_status, &0_u32.to_le_bytes()).await,
    ];
    let bus = ports.bus.clone();
    let (completion, destination, dma_interrupts) = transfer::run(ports, payload, channel).await?;
    management
        .release_channel(FunctionId::new(0), channel)
        .await?;
    bus.read(window_status, &mut ready).await?;
    Ok(Outcome {
        before,
        after,
        completion,
        destination,
        released: u32::from_le_bytes(ready),
        dma_interrupts: dma_interrupts.load(std::sync::atomic::Ordering::SeqCst),
    })
}

async fn channel_address(
    bus: &HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
    id: pdma_model_contract::dma::ChannelId,
) -> TestResult<u32> {
    let mut count = [0; 4];
    bus.read(
        bus.addresses().dma
            + u32::try_from(
                pdma_model_contract::dma::DmaRegisters::DISCOVERY.offset()
                    + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset(),
            )?,
        &mut count,
    )
    .await?;
    let count = usize::try_from(u32::from_le_bytes(count))?;
    Ok(bus
        .addresses()
        .dma
        .checked_add(u32::try_from(
            id.register_offset(count).ok_or("channel address")?,
        )?)
        .ok_or("channel address overflow")?)
}
