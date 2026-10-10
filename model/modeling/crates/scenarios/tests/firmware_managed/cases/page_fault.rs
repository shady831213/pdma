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
async fn shared_pasid_channel_tags_select_one_channel_and_retire_before_reset() -> TestResult {
    // Given: two channels share one PASID and fault on the same page.
    let input = page_fault::shared_space::Input {
        ids: [80, 81, 90],
        payload: vec![0x6d; 256],
        attributes: pasid(7),
        lifecycle: page_fault::shared_space::Lifecycle::Reset,
    };
    // When: each report authorizes only its source, then the handler retires before reset.
    let result = page_fault::shared_space::run(&input).await?;
    // Then: the peer waits for its own resume and stale handling cannot affect the new work.
    assert_shared_recovery(&input, &result)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn shared_pasid_channel_tags_select_one_channel_and_retire_before_reallocation() -> TestResult
{
    // Given: two channels share one PASID and fault on the same page.
    let input = page_fault::shared_space::Input {
        ids: [403, 219, 705],
        payload: (11_u8..=197).cycle().take(379).collect(),
        attributes: pasid(31),
        lifecycle: page_fault::shared_space::Lifecycle::Release,
    };
    // When: the owner handles tagged reports and retires handling before release/reallocation.
    let result = page_fault::shared_space::run(&input).await?;
    // Then: the reallocated channel completes only after its fresh report is handled.
    assert_shared_recovery(&input, &result)?;
    Ok(())
}

fn assert_shared_recovery(
    input: &page_fault::shared_space::Input,
    result: &page_fault::shared_space::Outcome,
) -> TestResult {
    let [first, second]: &[page_fault::FaultReport; 2] = result.faults.as_slice().try_into()?;
    assert_eq!(first.identity, second.identity);
    assert_eq!(first.identity.function, FunctionId::new(0));
    assert_eq!(first.identity.attributes, input.attributes);
    assert_eq!(first.address, second.address);
    assert_eq!(first.access, FaultAccess::Read);
    assert_eq!(second.access, FaultAccess::Read);
    assert_ne!(first.ch_tag, second.ch_tag);
    assert!(result.peer_stayed_parked);
    assert!(result.stale_rejected);
    assert!(result.foreign_rejected);
    let mut responses: Vec<_> = result
        .responses
        .iter()
        .map(|response| (response.id, response.status, response.bytes_completed()))
        .collect();
    responses.sort_by_key(|response| response.0);
    let mut ids = input.ids;
    ids.sort_unstable();
    let length = u64::try_from(input.payload.len())?;
    assert_eq!(
        responses,
        ids.map(|id| (id, ResponseStatus::Success, length))
    );
    assert_eq!(result.bytes, vec![input.payload.clone(); 2]);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn page_fault_vf_repairs_platform_reported_pasids_and_resumes_original_command() -> TestResult
{
    // Given: equal payload VAs in independent PASIDs and one VF-owned SG command.
    let input = page_fault::Input {
        id: 17,
        payload: (0_u8..=250).cycle().take(600).collect(),
        attributes: [ContextAttributes::DEFAULT, pasid(0), pasid(1), pasid(2)],
        splits: [300, 200],
        base: 0x1000.into(),
        fault_offset: 256,
    };
    // When: platform reports two missing pages and VF repairs each mapping before resuming.
    assert_isolated_recovery(&input).await
}

#[tokio::test(flavor = "current_thread")]
async fn page_fault_recovery_preserves_varied_command_and_address_space_inputs() -> TestResult {
    // Given: distinct command, payload, SG boundaries, PASIDs and fault address.
    let input = page_fault::Input {
        id: 0x1234,
        payload: (19_u8..=231).rev().cycle().take(913).collect(),
        attributes: [ContextAttributes::DEFAULT, pasid(9), pasid(57), pasid(103)],
        splits: [377, 513],
        base: 0x4000.into(),
        fault_offset: 512,
    };
    // When: the owner repairs and resumes each fault of the original SG command.
    assert_isolated_recovery(&input).await
}

async fn assert_isolated_recovery(input: &page_fault::Input) -> TestResult {
    let (result, foreign_resume) = page_fault::run(input).await?;
    // Then: the original ID completes exactly once, with all bytes and isolated channel access.
    assert_eq!(
        result
            .responses
            .iter()
            .map(|r| (r.id, r.status, r.bytes_completed()))
            .collect::<Vec<_>>(),
        vec![(
            input.id,
            ResponseStatus::Success,
            u64::try_from(input.payload.len())?
        )]
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
        [
            (input.attributes[2], FaultAccess::Read),
            (input.attributes[3], FaultAccess::Write)
        ]
        .map(|(attributes, access)| (
            FunctionId::new(1),
            attributes,
            input.base.raw() + input.fault_offset,
            access
        ))
    );
    assert_eq!(result.faulted, [true, true]);
    let [read, write]: &[page_fault::FaultReport; 2] = result.faults.as_slice().try_into()?;
    assert_eq!(read.ch_tag, write.ch_tag);
    assert_eq!(result.bytes, input.payload);
    assert_eq!(foreign_resume, Err(AccessError::Denied));
    assert_eq!(result.closed_resume, Some(Err(AccessError::Closed)));
    Ok(())
}

const fn pasid(pasid: u32) -> ContextAttributes {
    ContextAttributes {
        address_space: ContextAttributesAddressSpace::Pasid,
        pasid,
    }
}
