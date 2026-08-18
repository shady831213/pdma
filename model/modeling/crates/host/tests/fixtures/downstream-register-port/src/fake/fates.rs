use super::*;
use hardware_interface_runtime::{GateFault, MemoryRegion, ObservationGateError};
use pdma_model_contract::semantic::BufferId;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResult, MmioResultDetails};
use pdma_model_host::register::PdmaPreparedProposal;

pub(crate) async fn verify_fates(alignment: MemoryAlignment) -> TestResult<bool> {
    verify_provisioning(alignment).await?;
    verify_batch(alignment).await?;
    verify_claim().await?;
    Ok(true)
}

async fn verify_provisioning(alignment: MemoryAlignment) -> TestResult<()> {
    let storage_fake = FakeAccess::new(message());
    storage_fake.fail_next_allocation();
    let before = storage_fake.snapshot();
    let error = storage_fake
        .allocate_region(PdmaRegionRequest::new(1, alignment))
        .await
        .expect_err("storage exhaustion must win before allocation");
    let cause = PdmaProvisioningCause::StorageExhausted;
    assert_eq!(error, PdmaProvisioningError::Provisioning(cause));
    assert_eq!(Error::source(&error).and_then(|value| value.downcast_ref()), Some(&cause));
    assert_eq!(storage_fake.snapshot(), before);

    let fake = FakeAccess::new(message());
    let mapped = fake
        .allocate_region(PdmaRegionRequest::new(1, alignment))
        .await?;
    let mut one = [0];
    let address = mapped.base().checked_add(64).ok_or("unmapped address overflow")?;
    let before = fake.snapshot();
    let unmapped = MemoryAccess::read(&fake, address, alignment, &mut one)
        .expect_err("unmapped read must fail");
    assert_eq!(unmapped, MemoryAccessError::Unmapped { address });
    assert!(Error::source(&unmapped).is_none());
    assert_eq!(fake.snapshot(), before);

    let mut two = [0; 2];
    let region = mapped.storage().ok_or("mapped storage missing")?;
    let before = fake.snapshot();
    let crosses = MemoryAccess::read(&fake, mapped.base(), alignment, &mut two)
        .expect_err("crossing read must fail");
    assert_eq!(
        crosses,
        MemoryAccessError::CrossesRegion {
            address: mapped.base(),
            length: 2,
            region,
        }
    );
    assert!(Error::source(&crosses).is_none());
    assert_eq!(fake.snapshot(), before);

    let mismatched = MemoryRegion::try_new(mapped.base(), 2, alignment)?;
    let before = fake.snapshot();
    let invalid = PdmaMappedRegion::try_new(
        BufferId::new(91),
        mapped.base(),
        1,
        alignment,
        Some(mismatched),
    )
    .expect_err("inconsistent projection must fail");
    assert_eq!(invalid, PdmaProvisioningError::InvalidProjection);
    assert!(Error::source(&invalid).is_none());
    assert_eq!(fake.snapshot(), before);
    Ok(())
}

async fn verify_batch(alignment: MemoryAlignment) -> TestResult<()> {
    let cases = [
        PdmaBatchError::Access(MmioAccessError::WrongWidth {
            offset: 0x168,
            expected: 4,
            actual: 8,
        }),
        PdmaBatchError::Result(MmioResultDetails {
            result: MmioResult::InvalidQueue,
            details: [17, 18, 19],
        }),
        PdmaBatchError::InvalidShape,
        PdmaBatchError::Observation(ObservationGateError::Faulted {
            reason: GateFault::ObserverPanicked,
        }),
    ];
    for expected in cases {
        let fake = FakeAccess::new(message());
        let proposal = fake
            .prepare_regions(&[
                PdmaRegionRequest::new(128, alignment),
                PdmaRegionRequest::new(64, alignment),
                PdmaRegionRequest::new(128, alignment),
            ])
            .await?;
        let queue = activation(proposal.regions())?;
        fake.allocate_region(PdmaRegionRequest::new(1, alignment))
            .await?;
        fake.fail_next_batch(expected);
        let before = fake.snapshot();
        let actual = fake
            .bootstrap_queue(proposal, queue)
            .await
            .expect_err("injected typed fate must precede stale proposal");
        assert_eq!(actual, expected);
        assert_batch_source(actual, expected);
        assert_eq!(fake.snapshot(), before);
    }
    Ok(())
}

fn assert_batch_source(actual: PdmaBatchError, expected: PdmaBatchError) {
    match expected {
        PdmaBatchError::Access(source) => assert_eq!(
            Error::source(&actual).and_then(|value| value.downcast_ref()),
            Some(&source)
        ),
        PdmaBatchError::Observation(source) => assert_eq!(
            Error::source(&actual).and_then(|value| value.downcast_ref()),
            Some(&source)
        ),
        PdmaBatchError::Result(_) | PdmaBatchError::InvalidShape => {
            assert!(Error::source(&actual).is_none())
        }
        PdmaBatchError::Provisioning(source) => assert_eq!(
            Error::source(&actual).and_then(|value| value.downcast_ref()),
            Some(&source)
        ),
    }
}

async fn verify_claim() -> TestResult<()> {
    let cases = [
        PdmaClaimError::Access(MmioAccessError::ReadFromWriteOnly { offset: 0x220 }),
        PdmaClaimError::Observation(ObservationGateError::SequenceExhausted {
            next_sequence: u64::MAX,
            count: 11,
        }),
    ];
    for expected in cases {
        let fake = FakeAccess::new(message());
        fake.fail_next_claim(expected);
        let before = fake.snapshot();
        let actual = fake
            .claim_interrupt_exact()
            .await
            .expect_err("typed claim fate must precede ready notification");
        assert_eq!(actual, expected);
        match expected {
            PdmaClaimError::Access(source) => assert_eq!(
                Error::source(&actual).and_then(|value| value.downcast_ref()),
                Some(&source)
            ),
            PdmaClaimError::Observation(source) => assert_eq!(
                Error::source(&actual).and_then(|value| value.downcast_ref()),
                Some(&source)
            ),
        }
        assert_eq!(fake.snapshot(), before);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn canonical_fake_executes_every_typed_fate_without_mutation() -> TestResult<()> {
        assert!(verify_fates(MemoryAlignment::try_new(64)?).await?);
        Ok(())
    }
}
