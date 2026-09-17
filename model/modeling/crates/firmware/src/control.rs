mod resources;
mod rings;

use crate::Error;
use hardware_interface_runtime::{
    CpuOwner, MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess,
};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{
    ControlRegisters, ControlRequest, ControlRequestRaw, ControlResponse, ControlRingRegisters,
    FIRMWARE_STATUS_BASE, FirmwareBootError, FirmwareBootState, FirmwareBootValues, Ring,
    WINDOW_CONTROL_BASE,
};
use rings::Snapshot;
use std::{convert::Infallible, future::pending};

/// Boots with DMA access closed, then serves control rings on the firmware CPU.
///
/// Cancelling this program stops software, not accepted DMA work or hardware reset.
/// Retain all ring/application memory until the subsystem is safely stopped.
pub async fn main<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync>(
    bus: B,
    memory: M,
) -> Result<Infallible, Error<B::Error>> {
    let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
    let setup = owner
        .scope(bus.write(WINDOW_CONTROL_BASE, &0_u32.to_le_bytes()))
        .await
        .map_err(Error::Cpu)?;
    let report = match setup {
        Ok(()) => FirmwareBootValues::default().with_state(FirmwareBootState::Ready),
        Err(_) => FirmwareBootValues::default()
            .with_state(FirmwareBootState::Failed)
            .with_error(FirmwareBootError::WindowSetup),
    };
    owner
        .scope(bus.write(FIRMWARE_STATUS_BASE, &report.raw().to_le_bytes()))
        .await
        .map_err(Error::Cpu)?
        .map_err(Error::Bus)?;
    match setup {
        Ok(()) => owner
            .scope(serve(&bus, &memory))
            .await
            .map_err(Error::Cpu)?,
        Err(_) => pending().await,
    }
}

async fn serve<B: RegisterAccess + Sync, M: MemoryAccess + Sync>(
    bus: &B,
    memory: &M,
) -> Result<Infallible, Error<B::Error>> {
    let mut allocated = false;
    loop {
        tokio::task::yield_now().await;
        if rings::read_word(bus, ControlRegisters::ENABLE.offset()).await? == 0 {
            continue;
        }
        let response =
            Snapshot::<ControlResponse>::read(bus, ControlRegisters::RESPONSE.offset()).await?;
        if response.pointers.pending() == response.capacity() {
            continue;
        }
        let response_next = response
            .advance_producer(response.pointers, 1)
            .map_err(Error::Ring)?;
        let command =
            Snapshot::<ControlRequest>::read(bus, ControlRegisters::COMMAND.offset()).await?;
        if command.pointers.is_empty() {
            continue;
        }
        let command_next = command
            .advance_consumer(command.pointers, 1)
            .map_err(Error::Ring)?;
        let address = MemoryAddress::new(
            command
                .entry(command.pointers.consumer)
                .map_err(Error::Ring)?,
        );
        let request = memory
            .read_typed::<ControlRequestRaw>(address, MemoryAlignment::ONE)
            .map_err(Error::Memory)?
            .decode()
            .map_err(Error::Codec)?;
        rings::write_word(
            bus,
            ControlRegisters::COMMAND.offset() + ControlRingRegisters::CONSUMER.offset(),
            command_next.consumer,
        )
        .await?;
        let result = resources::execute(bus, &mut allocated, request).await?;
        let address = MemoryAddress::new(
            response
                .entry(response.pointers.producer)
                .map_err(Error::Ring)?,
        );
        memory
            .write_typed(
                address,
                MemoryAlignment::ONE,
                &result.encode().map_err(Error::Codec)?,
            )
            .map_err(Error::Memory)?;
        rings::write_word(
            bus,
            ControlRegisters::RESPONSE.offset() + ControlRingRegisters::PRODUCER.offset(),
            response_next.producer,
        )
        .await?;
    }
}
