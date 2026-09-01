use super::*;
use hardware_interface_runtime::RegisterAccess;

#[tokio::test(flavor = "current_thread")]
async fn direct_adapter_claim_parity_holds_for_twenty_five_real_schedules() -> TestResult<()> {
    // Given: every schedule creates a fresh real device core, observer, store, and adapter.
    let alignment = MemoryAlignment::try_new(64)?;
    for _ in 0..25 {
        // When: real work produces and the compound adapter claims one message.
        let result = run(alignment).await?;

        // Then: every literal Task 18 field and order is exact for every fresh schedule.
        assert!(result.notification_is_message);
        crate::claim::assert_exact(&result.observations)?;
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn canonical_adapter_preserves_every_mmio_fate_and_full_state() -> TestResult<()> {
    use pdma_model_contract::abi::mmio::MmioAccessError;
    use pdma_model_host::adapter::abi::PdmaRegisterAccessError;

    let observer = Arc::new(Observer::default());
    let (device, worker) = AsyncDevice::from_core_observed(
        DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default()),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let access = AsyncDeviceAccess::new(handle.clone());

    macro_rules! assert_fate {
        ($operation:expr, $expected:expr) => {{
            let before = (
                handle.snapshot().await,
                handle.memory_generation().await,
                lock(&observer.0).clone(),
            );
            let expected = $expected;
            let error = $operation.await.expect_err("canonical access must fail");
            assert_eq!(error, PdmaRegisterAccessError::Access(expected));
            assert_eq!(
                Error::source(&error).and_then(|source| source.downcast_ref::<MmioAccessError>()),
                Some(&expected)
            );
            let after = (
                handle.snapshot().await,
                handle.memory_generation().await,
                lock(&observer.0).clone(),
            );
            assert_eq!(before, after);
        }};
    }

    let mut four = [0; 4];
    let mut eight = [0; 8];
    assert_fate!(
        RegisterAccess::read(&access, 0xffc, &mut eight),
        MmioAccessError::UnmappedOffset { offset: 0xffc }
    );
    assert_fate!(
        RegisterAccess::read(&access, 0x00c, &mut four),
        MmioAccessError::ReservedOffset { offset: 0x00c }
    );
    assert_fate!(
        RegisterAccess::read(&access, 0x011, &mut eight),
        MmioAccessError::Misaligned {
            offset: 0x011,
            width: 8,
        }
    );
    assert_fate!(
        RegisterAccess::read(&access, 0x010, &mut four),
        MmioAccessError::WrongWidth {
            offset: 0x010,
            expected: 8,
            actual: 4,
        }
    );
    assert_fate!(
        RegisterAccess::read(&access, 0x020, &mut four),
        MmioAccessError::ReadFromWriteOnly { offset: 0x020 }
    );
    assert_fate!(
        RegisterAccess::write(&access, 0, &[0; 4]),
        MmioAccessError::WriteToReadOnly { offset: 0 }
    );
    assert_fate!(
        RegisterAccess::write(&access, 0x008, &[0, 0, 1, 0]),
        MmioAccessError::ReservedBits {
            offset: 0x008,
            bits: 0x10000,
        }
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
