use super::*;
use pdma_model_host::register::{PdmaPreparedProposal, PdmaRegisterPort};
use pdma_model_host::{DriverPort, ModelError, SoftwareCpu};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

#[tokio::test(flavor = "current_thread")]
async fn cancellation_and_clone_state_hold_when_scheduled_twenty_five_ways() -> TestResult<()> {
    // Given: each schedule starts with an independent non-PDMA capability and notification.
    for index in 0..25 {
        let alignment = MemoryAlignment::try_new(64)?;
        let fake = FakeAccess::new(message());
        let clone = fake.clone();
        let request = PdmaRegionRequest::new(u64::try_from(index + 1)?, alignment);

        // When: proposal, stale, register-lock, and compound-claim futures are dropped precommit.
        let before_drop = fake.snapshot();
        drop(fake.prepare_regions(&[request]).await?);
        assert_eq!(fake.snapshot(), before_drop);
        let stale = fake.prepare_regions(&[request]).await?;
        clone.allocate_region(request).await?;
        let before_stale = fake.snapshot();
        let rings = fake
            .prepare_regions(&[
                PdmaRegionRequest::new(128, alignment),
                PdmaRegionRequest::new(64, alignment),
                PdmaRegionRequest::new(128, alignment),
            ])
            .await?;
        assert!(
            fake.bootstrap_queue(stale, activation(rings.regions())?)
                .await
                .is_err()
        );
        assert_eq!(fake.snapshot(), before_stale);
        fake.pend_next_register();
        let bytes = [u8::try_from(index)?; 8];
        let pending_clone = clone.clone();
        let mut register = Box::pin(RegisterAccess::write(&pending_clone, 0x100, &bytes));
        assert!(poll_pending(&mut register));
        drop(register);
        assert_eq!(fake.snapshot(), before_stale);
        RegisterAccess::write(&clone, 0x100, &bytes).await?;
        fake.pend_next_claim();
        let before_claim = fake.snapshot();
        let mut claim = Box::pin(fake.claim_interrupt_exact());
        assert!(poll_pending(&mut claim));
        drop(claim);
        assert_eq!(fake.snapshot(), before_claim);

        // Then: a fresh claim consumes once and both clones observe the same committed mapping.
        let claimed = clone.claim_interrupt_exact().await?;
        crate::claim::assert_exact(crate::claim::observations(&claimed)?)?;
        assert_eq!(clone.snapshot().mappings, fake.snapshot().mappings);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn cpu_preflight_rejects_before_allocation_when_port_is_inactive() -> TestResult<()> {
    // Given: a canonical port is constructed within its owning CPU and then returned.
    let fake = FakeAccess::new(message());
    let owner = SoftwareCpu::new();
    let mut port = owner
        .run_main(async { PdmaRegisterPort::new(fake.clone()) })
        .await?;
    let before = fake.snapshot();

    // When: allocation is polled without the owning CPU.
    let result = port.allocate_buffer(8).await;

    // Then: CPU precedence rejects before provisioning mutates any fake state.
    assert_eq!(result, Err(ModelError::SoftwareCpuInactive));
    assert_eq!(fake.snapshot(), before);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn canonical_provisioning_fates_preserve_exact_state_and_sources() -> TestResult<()> {
    let alignment = MemoryAlignment::try_new(64)?;
    let exhausted = FakeAccess::new(message());
    super::helpers::lock(&exhausted.state).next_base = u64::MAX;
    let before_address = exhausted.snapshot();
    let address = exhausted
        .allocate_region(PdmaRegionRequest::new(1, alignment))
        .await
        .expect_err("address exhaustion must fail");
    assert_eq!(
        address,
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::AddressExhausted)
    );
    assert_eq!(
        Error::source(&address).and_then(|source| source.downcast_ref::<PdmaProvisioningCause>()),
        Some(&PdmaProvisioningCause::AddressExhausted)
    );
    assert_eq!(exhausted.snapshot(), before_address);

    let fake = FakeAccess::new(message());
    fake.fail_next_allocation();
    let before_storage = fake.snapshot();
    let storage = fake
        .allocate_region(PdmaRegionRequest::new(1, alignment))
        .await
        .expect_err("storage exhaustion must fail");
    assert_eq!(
        storage,
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::StorageExhausted)
    );
    assert_eq!(fake.snapshot(), before_storage);

    let mapped = fake
        .allocate_region(PdmaRegionRequest::new(1, alignment))
        .await?;
    let before_access = fake.snapshot();
    let mut one = [0; 1];
    let unmapped_address = mapped.base().checked_add(64).ok_or("address overflow")?;
    assert_eq!(
        MemoryAccess::read(&fake, unmapped_address, alignment, &mut one),
        Err(MemoryAccessError::Unmapped {
            address: unmapped_address
        })
    );
    let mut two = [0; 2];
    assert!(matches!(
        MemoryAccess::read(&fake, mapped.base(), alignment, &mut two),
        Err(MemoryAccessError::CrossesRegion { .. })
    ));
    assert_eq!(fake.snapshot(), before_access);

    assert_eq!(
        PdmaMappedRegion::try_new(
            mapped.object(),
            mapped.base(),
            0,
            alignment,
            mapped.storage(),
        ),
        Err(PdmaProvisioningError::InvalidProjection)
    );
    assert_eq!(fake.snapshot(), before_access);
    Ok(())
}

fn poll_pending<F: Future>(future: &mut Pin<Box<F>>) -> bool {
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    matches!(future.as_mut().poll(&mut context), Poll::Pending)
}
