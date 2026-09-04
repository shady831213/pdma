use super::support::{TestResult, read_u32, response, result, write_u32, write_u64};
use pdma_model_contract::abi::mmio::MmioAccessError;
use pdma_model_contract::abi::{AbiDiscovery, AbiLimits};
use pdma_model_device::AsyncDevice;

pub(super) async fn assert_global_commands() -> TestResult {
    run().await
}

async fn run() -> TestResult {
    // Given: a fresh device and independently typed ABI-v1 discovery and limits.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let discovery = AbiDiscovery::v1();
    let limits = AbiLimits::v1();
    assert_eq!(read_u32(&handle, 0x000).await?, 0x414d_4450);
    assert_eq!(read_u32(&handle, 0x004).await?, 1);

    // When: DISCOVER is issued with the typed version and no required feature subset.
    write_u32(&handle, 0x008, u32::from(discovery.version())).await?;
    write_u64(&handle, 0x010, 0).await?;
    write_u32(&handle, 0x020, 1).await?;

    // Then: the full response is replaced by canonical discovery values.
    let discover_response = response(&handle).await?;
    assert_eq!(result(&handle).await?, [1, 0, 0, 0]);
    assert_eq!(discover_response[0], u64::from(discovery.version()));
    assert_eq!(discover_response[2], discovery.features().bits());
    assert_eq!(discover_response[1], 0);
    assert_eq!(&discover_response[3..], &[0; 6]);

    // When: unsupported version, unknown features, and an unknown command are attempted.
    write_u32(&handle, 0x008, 2).await?;
    write_u32(&handle, 0x020, 1).await?;
    assert_eq!(result(&handle).await?, [0x100, 2, 0, 0]);
    assert_eq!(response(&handle).await?, discover_response);
    write_u32(&handle, 0x008, 1).await?;
    write_u64(&handle, 0x010, 1_u64 << 63).await?;
    write_u32(&handle, 0x020, 1).await?;
    assert_eq!(result(&handle).await?, [0x101, 1_u64 << 63, 0, 0]);
    assert_eq!(response(&handle).await?, discover_response);
    write_u32(&handle, 0x020, 0xfeed).await?;
    assert_eq!(result(&handle).await?, [0x11b, 0xfeed, 0, 0]);
    assert_eq!(response(&handle).await?, discover_response);

    // When: READ_LIMITS succeeds after semantic failures.
    write_u32(&handle, 0x020, 2).await?;

    // Then: unrelated response fields are zero and all five typed maxima replace the bank.
    assert_eq!(result(&handle).await?, [1, 0, 0, 0]);
    assert_eq!(
        response(&handle).await?,
        [
            0,
            0,
            0,
            0,
            u64::from(limits.max_ring_depth()),
            u64::from(limits.max_indirect_destinations()),
            u64::from(limits.max_transfer_bytes()),
            u64::from(limits.max_strided_rows()),
            u64::from(limits.max_stride()),
        ]
    );

    // When: malformed structural reads are repeated.
    let before = handle.snapshot().await;
    let prior = response(&handle).await?;
    let prior_result = result(&handle).await?;
    assert_eq!(
        handle.write_mmio(0x008, &0x1_0000_u32.to_le_bytes()).await,
        Err(MmioAccessError::ReservedBits {
            offset: 0x008,
            bits: 0x1_0000,
        })
    );
    for _ in 0..2 {
        assert_eq!(
            handle.read_mmio(0x040, 8).await,
            Err(MmioAccessError::WrongWidth {
                offset: 0x040,
                expected: 4,
                actual: 8,
            })
        );
    }

    // Then: structural failures change neither runner generation nor command state.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(response(&handle).await?, prior);
    assert_eq!(result(&handle).await?, prior_result);
    let typed = handle
        .__inspect_register_image_for_test(|root| -> TestResult<[u64; 9]> {
            let global = root.global()?;
            Ok([
                global.rsp_abi_version()?.value()?,
                global.rsp_run_state()?.value()?,
                global.rsp_features()?.value()?,
                global.rsp_epoch()?.value()?,
                global.max_ring_depth()?.value()?,
                global.max_indirect_destinations()?.value()?,
                global.max_transfer_bytes()?.value()?,
                global.max_strided_rows()?.value()?,
                global.max_stride()?.value()?,
            ])
        })
        .await?;
    assert_eq!(typed, prior);
    println!(
        "global result=[1,0,0,0] failures=[0x100,0x101,0x11b] response={prior:?} typed_raw=agree same_write_future=committed"
    );
    Ok(())
}
