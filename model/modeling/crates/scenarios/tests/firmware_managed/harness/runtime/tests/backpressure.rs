use super::super::RunError;
use super::{AccessError, Managed, control::rings};
use crate::harness::TestResult;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use hardware_interface_schema::{Decode, Encode, SchemaCodecError};
use pdma_model_contract::{
    CONTROL_CHANNEL_BASE, ControlRegisters, ControlRequest, ControlRequestOpcode,
    ControlResponseRaw, ControlResponseStatus, ControlRingRegisters, Ring, WINDOW_STATUS_BASE,
};
use pdma_model_host::{Management, RingConfig};
use std::{future::pending, sync::Arc, time::Duration};

const COMMAND_PRODUCER: usize =
    ControlRegisters::COMMAND.offset() + ControlRingRegisters::PRODUCER.offset();
const COMMAND_CONSUMER: usize =
    ControlRegisters::COMMAND.offset() + ControlRingRegisters::CONSUMER.offset();
const RESPONSE_PRODUCER: usize =
    ControlRegisters::RESPONSE.offset() + ControlRingRegisters::PRODUCER.offset();
const RESPONSE_CONSUMER: usize =
    ControlRegisters::RESPONSE.offset() + ControlRingRegisters::CONSUMER.offset();

async fn read_word(
    bus: &(impl RegisterAccess<Error = AccessError> + Sync),
    address: u32,
) -> TestResult<u32> {
    let mut bytes = [0; 4];
    bus.read(address, &mut bytes).await?;
    Ok(u32::from_le_bytes(bytes))
}

fn address(offset: usize) -> TestResult<u32> {
    Ok(CONTROL_CHANNEL_BASE + u32::try_from(offset)?)
}

#[tokio::test]
async fn backpressure_preserves_pending_request_until_host_releases_response_slot() -> TestResult {
    // Given: real firmware with a capacity-one response ring holding an acquisition result.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let cpu = SoftwareCpu::new();
    let program = cpu.run_main(async {
        let mut storage = rings(&allocator)?;
        storage.command = RingConfig::new(storage.command.base(), 1)?;
        storage.response = RingConfig::new(storage.response.base(), 1)?;
        let _client = Management::setup(host.bus.clone(), memory.clone(), storage).await?;
        memory.write_typed(
            MemoryAddress::new(storage.command.entry(0)?),
            MemoryAlignment::ONE,
            &ControlRequest {
                id: 1,
                opcode: ControlRequestOpcode::AcquireChannel,
                channel: 0,
            }
            .encode()?,
        )?;
        host.bus
            .write(address(COMMAND_PRODUCER)?, &1_u32.to_le_bytes())
            .await?;
        while read_word(&host.bus, address(RESPONSE_PRODUCER)?).await? != 1 {
            tokio::task::yield_now().await;
        }
        let first = memory
            .read_typed::<ControlResponseRaw>(
                MemoryAddress::new(storage.response.entry(0)?),
                MemoryAlignment::ONE,
            )?
            .decode()?;
        assert_eq!(
            (first.id, first.status),
            (1, ControlResponseStatus::Success)
        );
        // When: Host publishes a release while the response slot remains occupied.
        memory.write_typed(
            MemoryAddress::new(storage.command.entry(1)?),
            MemoryAlignment::ONE,
            &ControlRequest {
                id: 2,
                opcode: ControlRequestOpcode::ReleaseChannel,
                channel: 0,
            }
            .encode()?,
        )?;
        host.bus
            .write(address(COMMAND_PRODUCER)?, &2_u32.to_le_bytes())
            .await?;
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        // Then: firmware neither consumes nor executes release until space is returned.
        assert_eq!(read_word(&host.bus, address(COMMAND_CONSUMER)?).await?, 1);
        assert_eq!(read_word(&host.bus, address(RESPONSE_PRODUCER)?).await?, 1);
        assert_eq!(read_word(&host.bus, WINDOW_STATUS_BASE).await?, 1);
        host.bus
            .write(address(RESPONSE_CONSUMER)?, &1_u32.to_le_bytes())
            .await?;
        while read_word(&host.bus, address(RESPONSE_PRODUCER)?).await? != 2 {
            tokio::task::yield_now().await;
        }
        let released = memory
            .read_typed::<ControlResponseRaw>(
                MemoryAddress::new(storage.response.entry(1)?),
                MemoryAlignment::ONE,
            )?
            .decode()?;
        assert_eq!(
            (released.id, released.status, released.channel),
            (2, ControlResponseStatus::Success, 0)
        );
        assert_eq!(read_word(&host.bus, address(COMMAND_CONSUMER)?).await?, 2);
        assert_eq!(read_word(&host.bus, address(RESPONSE_CONSUMER)?).await?, 1);
        assert_eq!(read_word(&host.bus, WINDOW_STATUS_BASE).await?, 0);
        Ok(())
    });
    tokio::time::timeout(Duration::from_secs(3), system.run_until(program)).await??
}

async fn rejected_command_case(unmapped: bool) -> TestResult {
    // Given: a command queue pointing to an invalid record or unmapped memory.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let cpu = SoftwareCpu::new();
    let program = cpu.run_main(async {
        let mut storage = rings(&allocator)?;
        if unmapped {
            storage.command = RingConfig::new(0xdead_0000, 1)?;
        } else {
            memory.write(
                MemoryAddress::new(storage.command.entry(0)?),
                MemoryAlignment::ONE,
                &[1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0],
            )?;
        }
        let _client = Management::setup(host.bus.clone(), memory.clone(), storage).await?;
        // When: Host publishes the unreadable or malformed command for real firmware.
        host.bus
            .write(address(COMMAND_PRODUCER)?, &1_u32.to_le_bytes())
            .await?;
        pending::<TestResult>().await
    });
    let result = tokio::time::timeout(Duration::from_secs(3), system.run_until(program)).await?;
    // Then: the exact firmware error leaves the command unconsumed and allocation closed.
    if unmapped {
        assert!(
            matches!(result, Err(RunError::Firmware(pdma_model_firmware::Error::Memory(
            MemoryAccessError::Unmapped { address }
        ))) if address == MemoryAddress::new(0xdead_0000))
        );
    } else {
        assert!(matches!(
            result,
            Err(RunError::Firmware(pdma_model_firmware::Error::Codec(
                SchemaCodecError::UnknownTag { observed: 2, .. }
            )))
        ));
    }
    assert_eq!(read_word(&host.bus, address(COMMAND_PRODUCER)?).await?, 1);
    assert_eq!(read_word(&host.bus, address(COMMAND_CONSUMER)?).await?, 0);
    assert_eq!(read_word(&host.bus, address(RESPONSE_PRODUCER)?).await?, 0);
    assert_eq!(read_word(&host.bus, WINDOW_STATUS_BASE).await?, 0);
    Ok(())
}

#[tokio::test]
async fn backpressure_malformed_opcode_preserves_unconsumed_command() -> TestResult {
    rejected_command_case(false).await
}

#[tokio::test]
async fn backpressure_unmapped_command_preserves_unconsumed_command() -> TestResult {
    rejected_command_case(true).await
}
