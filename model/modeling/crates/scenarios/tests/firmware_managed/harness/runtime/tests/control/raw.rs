use super::memory::{QueueMemory, kick};
use super::*;
use hardware_interface_runtime::MemoryAccess;
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{ControlRequestOpcode, ControlResponseRaw, Ring};

#[tokio::test]
async fn acquisition_selects_channel_instead_of_using_ignored_request_field() {
    // Given: a Host using the wire protocol rather than the convenience API.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone()).unwrap();
    let cpu = SoftwareCpu::new();
    let program = cpu.run_main(async {
        let rings = rings(&allocator).unwrap();
        let _client = Management::setup(host.bus.clone(), memory.clone(), rings)
            .await
            .unwrap();
        let request = ControlRequest {
            id: 77,
            opcode: ControlRequestOpcode::AcquireChannel,
            channel: 7,
        };
        // When: a request with a nonzero ignored acquisition field is published.
        memory
            .write_typed(
                MemoryAddress::new(rings.command.entry(0).unwrap()),
                MemoryAlignment::ONE,
                &request.encode().unwrap(),
            )
            .unwrap();
        let queue = QueueMemory::new(&memory, rings.state);
        queue.publish_command(1).unwrap();
        kick(&host.bus).await.unwrap();
        queue.wait_response(&host.bus, 1).await.unwrap();
        let response = memory
            .read_typed::<ControlResponseRaw>(
                MemoryAddress::new(rings.response.entry(0).unwrap()),
                MemoryAlignment::ONE,
            )
            .unwrap()
            .decode()
            .unwrap();
        // Then: firmware returns its selected channel and the same request id.
        assert_eq!(response.id, 77);
        assert_eq!(response.status, ControlResponseStatus::Success);
        assert_eq!(response.channel, 0);
    });
    tokio::time::timeout(Duration::from_secs(3), system.run_until(program))
        .await
        .unwrap()
        .unwrap();
}
