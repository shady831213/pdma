use crate::harness::{
    TestResult,
    page_fault::{self, FaultAccess},
    runtime::AccessError,
};
use pdma_model_contract::{
    dma::ResponseStatus,
    memory::{ContextAttributes, ContextAttributesAddressSpace, FunctionId},
};

#[tokio::test(flavor = "current_thread")]
async fn shared_pasid_fault_tags_select_one_channel_and_retire_before_reset() -> TestResult {
    // Given: two channels share one PASID and fault on the same page.
    // When: each report authorizes only its source, then the handler retires before reset.
    let result = page_fault::shared_space::run(page_fault::shared_space::Lifecycle::Reset).await?;
    // Then: the peer waits for its own resume and stale handling cannot affect the new work.
    assert_shared_recovery(&result)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn shared_pasid_fault_tags_select_one_channel_and_retire_before_reallocation() -> TestResult {
    // Given: two channels share one PASID and fault on the same page.
    // When: the owner handles tagged reports and retires handling before release/reallocation.
    let result =
        page_fault::shared_space::run(page_fault::shared_space::Lifecycle::Release).await?;
    // Then: the reallocated channel completes only after its fresh report is handled.
    assert_shared_recovery(&result)?;
    Ok(())
}

fn assert_shared_recovery(result: &page_fault::shared_space::Outcome) -> TestResult {
    let [first, second]: &[page_fault::FaultReport; 2] = result.faults.as_slice().try_into()?;
    assert_eq!(first.identity, second.identity);
    assert_eq!(first.identity.function, FunctionId::new(0));
    assert_eq!(
        first.identity.attributes,
        ContextAttributes {
            address_space: ContextAttributesAddressSpace::Pasid,
            pasid: 7
        }
    );
    assert_eq!(first.address, second.address);
    assert_eq!(first.access, FaultAccess::Read);
    assert_eq!(second.access, FaultAccess::Read);
    assert_ne!(first.tag, second.tag);
    assert!(result.peer_stayed_parked);
    assert!(result.stale_rejected);
    assert!(result.foreign_rejected);
    let mut responses: Vec<_> = result
        .responses
        .iter()
        .map(|response| (response.id, response.status, response.bytes_completed()))
        .collect();
    responses.sort_by_key(|response| response.0);
    assert_eq!(
        responses,
        [
            (80, ResponseStatus::Success, 256),
            (81, ResponseStatus::Success, 256),
            (90, ResponseStatus::Success, 256)
        ]
    );
    assert_eq!(result.bytes, vec![vec![0x6d; 256]; 2]);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn page_fault_vf_repairs_platform_reported_pasids_and_resumes_original_command() -> TestResult
{
    // Given: equal payload VAs in independent PASIDs and one VF-owned SG command.
    // When: platform reports two missing pages and VF repairs each mapping before resuming.
    let (result, foreign_resume) = page_fault::run().await?;
    // Then: the original ID completes exactly once, with all bytes and isolated channel access.
    assert_eq!(
        result
            .responses
            .iter()
            .map(|r| (r.id, r.status, r.bytes_completed()))
            .collect::<Vec<_>>(),
        vec![(17, ResponseStatus::Success, 600)]
    );
    assert_eq!(
        result
            .faults
            .iter()
            .map(|fault| (
                fault.identity.function,
                fault.identity.attributes,
                fault.address.raw(),
                fault.access
            ))
            .collect::<Vec<_>>(),
        [(1, FaultAccess::Read), (2, FaultAccess::Write)].map(|(pasid, access)| (
            FunctionId::new(1),
            ContextAttributes {
                address_space: ContextAttributesAddressSpace::Pasid,
                pasid
            },
            0x1100,
            access
        ))
    );
    assert_eq!(result.faulted, [true, true]);
    assert_eq!(
        result.bytes,
        (0_u8..=250).cycle().take(600).collect::<Vec<_>>()
    );
    assert_eq!(foreign_resume, Err(AccessError::Denied));
    assert_eq!(result.closed_resume, Some(Err(AccessError::Closed)));
    Ok(())
}
