use super::{client, control::rings, *};
use hardware_interface_runtime::{MemoryAddress, SharedMemoryAllocator};
use pdma_model_contract::dma::{ChannelAdmin, ChannelId, DmaControl, DmaDiscovery, DmaRegisters};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::Management;
use std::time::Duration;

#[tokio::test]
async fn granting_data_never_exposes_global_or_channel_administration() {
    // Given: two channels behind real managed runtime permissions.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, host) = Managed::with_channels(allocator.memory_access(), 2).unwrap();
    let program = client::run(host.runtime, async |interrupt| {
        let mut management = Management::setup(
            host.bus.clone(),
            allocator.memory_access(),
            client::config(rings(&allocator).unwrap(), &interrupt),
        )
        .await
        .unwrap();
        let id = management
            .acquire_channel(FunctionId::new(0))
            .await
            .unwrap();
        // When: a granted Host attempts every administrative reset and global control.
        for offset in [
            (pdma_model_contract::dma::DmaDiscovery::EXTENT
                + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
                + DmaControl::RESET.offset(),
            (pdma_model_contract::dma::DmaDiscovery::EXTENT
                + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
                + DmaControl::IRQ_ENABLE.offset(),
            id.admin_offset(2).unwrap() + ChannelAdmin::RESET.offset(),
            ChannelId::new(1).admin_offset(2).unwrap() + ChannelAdmin::RESET.offset(),
        ] {
            let mut bytes = [0xa5; 4];
            let address = u32::try_from(offset).unwrap();
            assert!(matches!(
                host.bus.read(address, &mut bytes).await,
                Err(AccessError::Denied)
            ));
            assert!(matches!(
                host.bus.write(address, &1_u32.to_le_bytes()).await,
                Err(AccessError::Denied)
            ));
            assert_eq!(bytes, [0xa5; 4]);
        }
        // Then: discovery and granted data remain usable, but the peer stays closed.
        let discovery =
            u32::try_from(DmaRegisters::DISCOVERY.offset() + DmaDiscovery::CHANNEL_NUM.offset())
                .unwrap();
        let mut count = [0; 4];
        host.bus.read(discovery, &mut count).await.unwrap();
        assert_eq!(u32::from_le_bytes(count), 2);
        assert!(matches!(
            host.bus.write(discovery, &9_u32.to_le_bytes()).await,
            Err(AccessError::Denied)
        ));
        host.bus
            .read(
                u32::try_from(id.register_offset(2).unwrap()).unwrap(),
                &mut [0; 8],
            )
            .await
            .unwrap();
        assert!(matches!(
            host.bus
                .read(
                    u32::try_from(ChannelId::new(1).register_offset(2).unwrap()).unwrap(),
                    &mut [0; 8]
                )
                .await,
            Err(AccessError::Closed)
        ));
    });
    tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn platform_rejects_overlapping_and_unaligned_apertures() {
    // Given: the platform's explicit address map.
    let base = crate::harness::runtime::platform::ADDRESSES;
    // When: any management block aliases DMA or violates its schema alignment.
    for addresses in [
        pdma_model_software::control::ManagementAddresses {
            control: base.dma,
            ..base
        },
        pdma_model_software::control::ManagementAddresses {
            firmware: base.windows,
            ..base
        },
        pdma_model_software::control::ManagementAddresses {
            windows: base.windows + 1,
            ..base
        },
        pdma_model_software::control::ManagementAddresses {
            dma: u32::MAX - 7,
            ..base
        },
    ] {
        let result = Managed::with_addresses(SharedMemory::new(), 2, addresses);
        // Then: no ambiguous platform assembly is created.
        assert!(result.is_err());
    }
}
