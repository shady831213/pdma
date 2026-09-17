use super::super::RunError;
use super::{
    AccessError, Managed,
    control::{
        memory::{QueueMemory, kick, poll_once},
        rings,
    },
};
use crate::harness::TestResult;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use hardware_interface_schema::{Decode, Encode, SchemaCodecError};
use pdma_model_contract::{
    ControlRequest, ControlRequestOpcode, ControlResponseRaw, ControlResponseStatus, Ring,
    WINDOW_STATUS_BASE,
};
use pdma_model_host::{Management, RingConfig};
use pdma_model_software::ring::SoftRingError;
use std::{future::pending, pin::pin, sync::Arc, time::Duration};

async fn read_word(
    bus: &(impl RegisterAccess<Error = AccessError> + Sync),
    address: u32,
) -> TestResult<u32> {
    let mut bytes = [0; 4];
    bus.read(address, &mut bytes).await?;
    Ok(u32::from_le_bytes(bytes))
}

#[tokio::test]
async fn backpressure_preserves_pending_request_until_host_releases_response_slot() -> TestResult {
    // Given: real firmware with a capacity-one response ring holding an acquisition result.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let mut running = pin!(system.run());
    assert!(poll_once(running.as_mut()).await.is_pending());
    let cpu = SoftwareCpu::new();
    let program = cpu.run_main(async {
        let mut storage = rings(&allocator)?;
        storage.command = RingConfig::new(storage.command.base(), 1)?;
        storage.response = RingConfig::new(storage.response.base(), 1)?;
        let _client = Management::setup(host.bus.clone(), memory.clone(), storage).await?;
        let queue = QueueMemory::new(&memory, storage.state);
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
        queue.publish_command(1)?;
        kick(&host.bus).await?;
        tokio::select! {
            result = running.as_mut() => panic!("system stopped: {result:?}"),
            result = queue.wait_response(&host.bus, 1) => result?,
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
        queue.publish_command(2)?;
        kick(&host.bus).await?;
        assert!(poll_once(running.as_mut()).await.is_pending());
        // Then: firmware neither consumes nor executes release until space and a kick arrive.
        assert_eq!(queue.read()?.command.consumer, 1);
        assert_eq!(queue.read()?.response.producer, 1);
        assert_eq!(read_word(&host.bus, WINDOW_STATUS_BASE).await?, 1);
        queue.consume_response(1)?;
        assert!(poll_once(running.as_mut()).await.is_pending());
        assert_eq!(queue.read()?.command.consumer, 1);
        assert_eq!(queue.read()?.response.producer, 1);
        kick(&host.bus).await?;
        tokio::select! {
            result = running.as_mut() => panic!("system stopped: {result:?}"),
            result = queue.wait_response(&host.bus, 2) => result?,
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
        assert_eq!(queue.read()?.command.consumer, 2);
        assert_eq!(queue.read()?.response.consumer, 1);
        assert_eq!(read_word(&host.bus, WINDOW_STATUS_BASE).await?, 0);
        Ok(())
    });
    tokio::time::timeout(Duration::from_secs(3), program).await?
}

async fn rejected_command_case(unmapped: bool) -> TestResult {
    // Given: a command queue pointing to an invalid record or unmapped memory.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
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
    let queue = QueueMemory::new(&memory, storage.state);
    let cpu = SoftwareCpu::new();
    let program = cpu.run_main(async {
        let _client = Management::setup(host.bus.clone(), memory.clone(), storage).await?;
        // When: Host publishes the unreadable or malformed command for real firmware.
        queue.publish_command(1)?;
        kick(&host.bus).await?;
        pending::<TestResult>().await
    });
    let result = tokio::time::timeout(Duration::from_secs(3), system.run_until(program)).await?;
    // Then: the exact firmware error leaves the command unconsumed and allocation closed.
    if unmapped {
        assert!(matches!(result,
            Err(RunError::Firmware(pdma_model_firmware::Error::SoftRing(SoftRingError::Memory(
                MemoryAccessError::Unmapped { address }
            )))) if address == MemoryAddress::new(0xdead_0000)));
    } else {
        assert!(matches!(
            result,
            Err(RunError::Firmware(pdma_model_firmware::Error::SoftRing(
                SoftRingError::Codec(SchemaCodecError::UnknownTag { observed: 2, .. })
            )))
        ));
    }
    assert_eq!(queue.read()?.command.producer, 1);
    assert_eq!(queue.read()?.command.consumer, 0);
    assert_eq!(queue.read()?.response.producer, 0);
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
