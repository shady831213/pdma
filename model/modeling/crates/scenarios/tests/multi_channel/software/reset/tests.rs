use hardware_interface_memory::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
    SharedMemoryAllocator,
};
use hardware_interface_runtime::{InterruptRuntime, SoftwareCpu};
use std::sync::Arc;

use super::{Buffers, Ports, wait_boundary};
use crate::{Scenario, TestResult, hardware};

struct Fixture {
    allocator: SharedMemoryAllocator,
    buffers: Vec<Buffers>,
    readable: Arc<SharedMemory>,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let allocator =
            SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
        let buffers = (0..3)
            .map(|index| Buffers::allocate(&allocator, index))
            .collect::<TestResult<_>>()?;
        Ok(Self {
            allocator,
            buffers,
            readable: Arc::new(SharedMemory::new()),
        })
    }

    fn install_byte(&self, address: MemoryAddress, value: u8) -> TestResult {
        self.readable
            .add_regions(&[MemoryRegion::try_new(address, 1, MemoryAlignment::ONE)?])?;
        self.readable
            .write(address, MemoryAlignment::ONE, &[value])?;
        Ok(())
    }

    async fn wait(&self, scenario: Scenario) -> TestResult<Option<(u32, u32)>> {
        let memory = Arc::clone(&self.readable).into();
        let (model, _stage) = hardware::assemble(memory)?;
        let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
        let execution = runtime.run_main(async |interrupts| {
            let ports = Ports {
                bus: model.clone(),
                memory: Arc::clone(&self.readable).into(),
                allocator: self.allocator.clone(),
                interrupts,
            };
            wait_boundary(&ports, &self.buffers, scenario).await
        });
        tokio::select! {
            biased;
            result = execution => result?.0,
            () = async { for _ in 0..100 { tokio::task::yield_now().await; } } => {
                Err("boundary polling exhausted its scheduling budget".into())
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn inflight_boundary_needs_only_first_destination_byte() -> TestResult {
    for scenario in [
        Scenario::CopyInFlight,
        Scenario::QueuedChannel,
        Scenario::WholeDevice,
    ] {
        // Given: only A's first destination byte is mapped and contains the copied value.
        let fixture = Fixture::new()?;
        let first = fixture.buffers.first().ok_or("missing A")?;
        fixture.install_byte(first.destination, first.value)?;
        // When: software waits for the in-flight boundary.
        let result = fixture.wait(scenario).await?;
        // Then: no other destination bytes are required.
        assert_eq!(result, None);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn completion_boundary_needs_only_fence_storage() -> TestResult {
    // Given: only the last byte of each peer's first destination is mapped and copied.
    let fixture = Fixture::new()?;
    for peer in &fixture.buffers {
        let address = MemoryAddress::new(peer.fence.0.target.address);
        fixture.readable.add_regions(&[MemoryRegion::try_new(
            address,
            8,
            MemoryAlignment::ONE,
        )?])?;
        fixture.readable.write(
            address,
            MemoryAlignment::ONE,
            &peer.fence.0.value.to_le_bytes(),
        )?;
    }
    // When: software waits for peers to finish before checking A's cursors.
    let result = fixture.wait(Scenario::SuccessWithoutEvents).await?;
    // Then: neither A nor the peers' other bytes need to be readable.
    assert_eq!(result, Some((0, 0)));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn boundary_waits_for_the_selected_byte_to_match() -> TestResult {
    // Given: A's boundary byte is readable but still contains its initial value.
    let fixture = Fixture::new()?;
    let first = fixture.buffers.first().ok_or("missing A")?;
    fixture.install_byte(first.destination, 0)?;
    let mut waiting = std::pin::pin!(fixture.wait(Scenario::CopyInFlight));
    // When: polling starts before the destination byte is copied.
    tokio::select! {
        biased;
        result = &mut waiting => panic!("boundary returned before copy: {result:?}"),
        () = tokio::task::yield_now() => {}
    }
    fixture
        .readable
        .write(first.destination, MemoryAlignment::ONE, &[first.value])?;
    // Then: it stays pending until the selected byte has the copied value.
    assert_eq!(waiting.await?, None);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn boundary_propagates_the_selected_byte_memory_error() -> TestResult {
    for scenario in [Scenario::CopyInFlight, Scenario::SuccessWithoutEvents] {
        // Given: the selected byte is unmapped; backpressure's earlier peer is complete.
        let fixture = Fixture::new()?;
        let address = match scenario {
            Scenario::SuccessWithoutEvents => MemoryAddress::new(
                fixture
                    .buffers
                    .first()
                    .ok_or("missing A")?
                    .fence
                    .0
                    .target
                    .address,
            ),
            Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::WholeDevice => {
                fixture.buffers.first().ok_or("missing A")?.destination
            }
        };
        // When: software polls the required byte.
        let result = fixture.wait(scenario).await;
        // Then: the exact backend address error reaches its caller.
        let error = result.expect_err("unmapped boundary byte must fail");
        assert_eq!(
            error.downcast_ref::<MemoryAccessError>(),
            Some(&MemoryAccessError::Unmapped { address })
        );
    }
    Ok(())
}
