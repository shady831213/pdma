mod program;

use super::{Managed, client, control::rings};
use hardware_interface_pipeline::{AsyncExecutor, HardwareDriver};
use hardware_interface_runtime::{
    InterruptRuntime, SharedMemory, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{
    dma::{CapabilitiesValues, ChannelId, EventCode},
    memory::FunctionId,
};
use pdma_model_dma::{Dma, DmaConfig};
use pdma_model_host::{Management, runtime::Model};
use pdma_model_software::control::ControlIsr;
use program::{Outcome, Ports, TestResult};
use std::{sync::Arc, time::Duration};

fn verify(outcome: &Outcome, supported: bool) {
    assert_eq!(
        outcome.fill,
        [
            0x11, 0x22, 0x33, 0x44, 0x11, 0x22, 0x33, 0x44, 0x11, 0x22, 0x33
        ]
    );
    assert_eq!(outcome.permits, 1);
    assert_eq!(outcome.fence, supported);
    assert_eq!(outcome.trap, supported);
    if supported {
        assert_eq!(outcome.word, 12);
        assert_eq!(outcome.event, None);
    } else {
        assert_eq!(outcome.word, 7);
        let event = outcome.event.expect("unsupported Atomic event");
        assert_eq!(
            (event.id, event.code, event.bytes_completed()),
            (5, EventCode::Unsupported, 0)
        );
    }
}

#[tokio::test]
async fn direct_commands_release_wait_for_peer_and_publish_fence() -> TestResult {
    for (supported, semaphore_first) in [(false, false), (false, true), (true, false)] {
        // Given: two channels and software-owned storage with selected atomic capabilities.
        let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
        let dma = Dma::with_config(
            allocator.memory_access(),
            DmaConfig {
                channels: 2,
                capabilities: CapabilitiesValues::default()
                    .with_atomic32(supported)
                    .unwrap()
                    .with_atomic64(supported)
                    .unwrap(),
            },
        )?;
        let mut hardware = dma.hardware();
        let model = Model::new(dma);
        let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
        let executor = AsyncExecutor::new(model.clone());
        let mut driver = HardwareDriver::new();
        // When: one channel waits and its peer publishes the dependency.
        let outcome = tokio::time::timeout(Duration::from_secs(3), async {
            tokio::select! {
                result = runtime.run_main::<ControlIsr, _>(async |_| program::run(Ports { bus: model.clone(), allocator, channels: [ChannelId::new(0), ChannelId::new(1)] }, (supported, semaphore_first)).await) => result.map(|(outcome, _)| outcome),
                result = executor.drive(&mut hardware, &mut driver) => panic!("hardware exited: {result:?}"),
            }
        }).await???;
        // Then: software observes ordered data and Fence/Trap or an explicit unsupported event.
        verify(&outcome, supported);
    }
    Ok(())
}

#[tokio::test]
async fn managed_commands_preserve_firmware_grants_and_atomic_capabilities() {
    for (supported, semaphore_first) in [(false, false), (false, true), (true, false)] {
        // Given: firmware-managed channels and a PF-owned control service.
        let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
        let (system, host) = Managed::with_dma_config(
            allocator.memory_access(),
            DmaConfig {
                channels: 2,
                capabilities: CapabilitiesValues::default()
                    .with_atomic32(supported)
                    .unwrap()
                    .with_atomic64(supported)
                    .unwrap(),
            },
        )
        .unwrap();
        let main = client::run(host.runtime, async |interrupt| {
            let mut management = Management::setup(
                host.bus.clone(),
                allocator.memory_access(),
                client::config(rings(&allocator).unwrap(), &interrupt),
            )
            .await
            .unwrap();
            let owner = FunctionId::new(0);
            let channels = [
                management.acquire_channel(owner).await.unwrap(),
                management.acquire_channel(owner).await.unwrap(),
            ];
            // When: ordinary channel software runs the same scalar and synchronization program.
            let outcome = program::run(
                Ports {
                    bus: host.bus,
                    allocator,
                    channels,
                },
                (supported, semaphore_first),
            )
            .await
            .unwrap();
            // Then: grants route all effects, errors stop Fence, and firmware can release both channels.
            verify(&outcome, supported);
            for id in channels {
                management.release_channel(owner, id).await.unwrap();
            }
        });
        tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(main)))
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn reset_retires_wait_without_publishing_cancelled_fence() -> TestResult {
    // Given: a retained false Wait, a queued Fence and an independent peer.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let dma = Dma::new(allocator.memory_access(), 2)?;
    let mut hardware = dma.hardware();
    let model = Model::new(dma);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
    let executor = AsyncExecutor::new(model.clone());
    let mut driver = HardwareDriver::new();
    // When: software observes peer progress and resets the waiting channel.
    tokio::time::timeout(Duration::from_secs(3), async {
        tokio::select! {
            result = runtime.run_main::<ControlIsr, _>(async |_| program::waiting::run(Ports { bus: model.clone(), allocator, channels: [ChannelId::new(0), ChannelId::new(1)] }).await) => result.map(|(outcome, _)| outcome),
            result = executor.drive(&mut hardware, &mut driver) => panic!("hardware exited: {result:?}"),
        }
    }).await???;
    // Then: later dependency repair cannot publish the cancelled Fence.
    Ok(())
}
