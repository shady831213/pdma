mod claim;
mod fake;
mod real;

use fake::{FakeAccess, activation, message};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use pdma_model_contract::abi::RingCursor;
use pdma_model_host::register::{
    PdmaBatchAccess, PdmaDmaWrite, PdmaExactInterruptClaim, PdmaMemoryProvisioning,
    PdmaPreparedProposal, PdmaRegionRequest, PdmaSubmissionBatch,
};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> TestResult {
    let alignment = MemoryAlignment::try_new(64)?;
    let fake = FakeAccess::new(message());
    let fake_nonempty = fake
        .allocate_region(PdmaRegionRequest::new(8, alignment))
        .await?;
    let fake_zero = fake
        .allocate_region(PdmaRegionRequest::new(0, alignment))
        .await?;

    let rings = fake
        .prepare_regions(&[
            PdmaRegionRequest::new(128, alignment),
            PdmaRegionRequest::new(64, alignment),
            PdmaRegionRequest::new(128, alignment),
        ])
        .await?;
    let activation = activation(rings.regions())?;
    fake.bootstrap_queue(rings, activation).await?;
    let indirect = fake
        .prepare_regions(&[PdmaRegionRequest::new(16, alignment)])
        .await?;
    let descriptor = PdmaDmaWrite::new(MemoryAddress::new(0x1_0000), vec![7; 64]);
    let indirect_write = PdmaDmaWrite::new(indirect.regions()[0].base(), vec![9; 16]);
    fake.submit_queue(PdmaSubmissionBatch::new(
        activation.queue(),
        RingCursor::new(0),
        descriptor,
        Some((indirect, indirect_write)),
    ))
    .await?;

    let before_claim_cancel = fake.snapshot();
    fake.pend_next_claim();
    let mut cancelled_claim = Box::pin(fake.claim_interrupt_exact());
    let claim_cancelled =
        poll_pending(&mut cancelled_claim) && fake.snapshot() == before_claim_cancel;
    drop(cancelled_claim);
    let fake_claim = fake.claim_interrupt_exact().await?;
    let fake_claim_observations = claim::observations(&fake_claim)?;
    claim::assert_exact(fake_claim_observations)?;

    let before_lock_cancel = fake.snapshot();
    fake.pend_next_register();
    let cancelled_access = fake.clone();
    let cancelled_bytes = [0; 8];
    let mut cancelled_register = Box::pin(RegisterAccess::write(
        &cancelled_access,
        0x100,
        &cancelled_bytes,
    ));
    let lock_cancelled =
        poll_pending(&mut cancelled_register) && fake.snapshot() == before_lock_cancel;
    drop(cancelled_register);
    RegisterAccess::write(&fake, 0x100, &[0; 8]).await?;

    let before_drop = fake.snapshot();
    let dropped = fake
        .prepare_regions(&[PdmaRegionRequest::new(32, alignment)])
        .await?;
    drop(dropped);
    let after_drop = fake.snapshot();
    let batch_dropped = before_drop == after_drop;

    let stale = fake
        .prepare_regions(&[PdmaRegionRequest::new(32, alignment)])
        .await?;
    fake.allocate_region(PdmaRegionRequest::new(1, alignment))
        .await?;
    let before_stale = fake.snapshot();
    let stale_error = fake
        .bootstrap_queue(stale, activation)
        .await
        .expect_err("stale proposal must fail");
    let after_stale = fake.snapshot();
    let stale_proposal = matches!(
        stale_error,
        pdma_model_host::register::PdmaBatchError::Provisioning(
            pdma_model_host::register::PdmaProvisioningError::Provisioning(
                pdma_model_host::register::PdmaProvisioningCause::StaleProposal { .. }
            )
        )
    ) && Error::source(&stale_error)
        .and_then(Error::source)
        .is_some()
        && before_stale == after_stale;
    let before_failure = fake.snapshot();
    fake.fail_next_allocation();
    let allocation_error = fake
        .allocate_region(PdmaRegionRequest::new(4, alignment))
        .await
        .expect_err("allocation must fail");
    let after_failure = fake.snapshot();
    let allocation_failure = matches!(
        allocation_error,
        pdma_model_host::register::PdmaProvisioningError::Provisioning(
            pdma_model_host::register::PdmaProvisioningCause::StorageExhausted
        )
    ) && Error::source(&allocation_error).is_some()
        && after_failure == before_failure;

    let mut malformed = [0; 4];
    let before_offset = fake.snapshot();
    let offset_error = RegisterAccess::read(&fake, 0x1000, &mut malformed)
        .await
        .expect_err("offset must fail");
    let after_offset = fake.snapshot();
    let malformed_offset = matches!(
        offset_error,
        pdma_model_host::register::PdmaRegisterAccessError::Access(
            pdma_model_contract::abi::mmio::MmioAccessError::UnmappedOffset { offset: 0x1000 }
        )
    ) && Error::source(&offset_error).is_some()
        && before_offset == after_offset;
    let mut malformed_width = [0; 3];
    let before_width = fake.snapshot();
    let width_error = RegisterAccess::read(&fake, 0, &mut malformed_width)
        .await
        .expect_err("width must fail");
    let after_width = fake.snapshot();
    let malformed_width = matches!(
        width_error,
        pdma_model_host::register::PdmaRegisterAccessError::Access(
            pdma_model_contract::abi::mmio::MmioAccessError::WrongWidth { actual: 3, .. }
        )
    ) && Error::source(&width_error).is_some()
        && before_width == after_width;

    let real = real::run(alignment).await?;
    claim::assert_exact(&real.observations)?;
    assert_eq!(fake_claim_observations, real.observations);
    let state = fake.snapshot();
    let exhaustive_fates = fake::verify_fates(alignment).await?;
    let mappings_unchanged = batch_dropped
        && allocation_failure
        && stale_proposal
        && before_offset == after_offset
        && before_width == after_width
        && claim_cancelled
        && lock_cancelled
        && exhaustive_fates;
    let bytes_unchanged = before_drop.bytes == after_drop.bytes;
    let notifications_unchanged = claim_cancelled;
    let typed_causes = stale_proposal
        && allocation_failure
        && malformed_offset
        && malformed_width
        && exhaustive_fates;
    let behavior_equal = claim::is_notification(&fake_claim) && real.notification_is_message;

    assert!(fake_nonempty.storage().is_some());
    assert!(fake_zero.storage().is_none() && fake_zero.length() == 0);
    let fake_storage = fake_nonempty
        .storage()
        .ok_or("fake nonempty storage missing")?;
    assert_eq!(
        (
            fake_storage.base(),
            fake_storage.length(),
            fake_storage.alignment()
        ),
        (
            fake_nonempty.base(),
            fake_nonempty.length(),
            fake_nonempty.alignment()
        )
    );
    let mut empty = [];
    assert!(MemoryAccess::read(&fake, fake_zero.base(), alignment, &mut empty).is_ok());
    assert!(!state.bytes.contains_key(&fake_zero.base()));
    assert!(
        MemoryAccess::read(
            &fake,
            fake_zero
                .base()
                .checked_add(1)
                .ok_or("zero base overflow")?,
            alignment,
            &mut empty
        )
        .is_err()
    );
    assert!(real.nonempty_storage && !real.zero_storage);
    assert!(
        state.queue_staged
            && state.indirect_staged
            && state.submission_committed
            && state.retirement
    );
    assert!(claim_cancelled && lock_cancelled && batch_dropped && mappings_unchanged);
    assert!(bytes_unchanged && notifications_unchanged && typed_causes && behavior_equal);
    assert_eq!(real.claim_observations, 11);
    assert!(real.direct_origin);

    println!(
        "fake_nonempty_storage={};fake_zero_storage={};fake_zero_length={};adapter_nonempty_storage={};adapter_zero_storage={};queue_staged={};indirect_staged={};submission_committed={};retirement={};compound_claim={};claim_observations={};direct_origin={};behavior_equal={}",
        fake_nonempty.storage().is_some(),
        fake_zero.storage().is_some(),
        fake_zero.length(),
        real.nonempty_storage,
        real.zero_storage,
        state.queue_staged,
        state.indirect_staged,
        state.submission_committed,
        state.retirement,
        claim::label(&fake_claim),
        real.claim_observations,
        real.direct_origin,
        behavior_equal
    );
    println!(
        "stale_proposal={};allocation_failure={};malformed_offset={};malformed_width={};lock_cancelled={};claim_cancelled={};batch_dropped={};mappings_unchanged={};bytes_unchanged={};notifications_unchanged={};typed_causes={}",
        stale_proposal,
        allocation_failure,
        malformed_offset,
        malformed_width,
        lock_cancelled,
        claim_cancelled,
        batch_dropped,
        mappings_unchanged,
        bytes_unchanged,
        notifications_unchanged,
        typed_causes
    );
    Ok(())
}

fn poll_pending<F: Future>(future: &mut Pin<Box<F>>) -> bool {
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    matches!(future.as_mut().poll(&mut context), Poll::Pending)
}
