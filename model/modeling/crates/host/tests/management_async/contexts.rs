use super::support::{Fixture, response};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::Error;
use pdma_model_software::ring::SoftRing;

#[tokio::test]
async fn default_and_pasid_zero_are_distinct_bounded_creation_requests() {
    // Given: two available control slots and no firmware task.
    let fixture = Fixture::new().unwrap();
    let mut client = fixture.setup().await.unwrap();
    let default = pdma_model_contract::memory::ContextAttributes::DEFAULT;
    let pasid = pdma_model_contract::memory::ContextAttributes {
        address_space: pdma_model_contract::memory::ContextAttributesAddressSpace::Pasid,
        pasid: 0,
    };
    // When: Host creates both address spaces without awaiting their completions.
    let first = client
        .submit_create_context(FunctionId::new(0), default)
        .await
        .unwrap();
    let second = client
        .submit_create_context(FunctionId::new(0), pasid)
        .await
        .unwrap();
    // Then: real queue records retain distinct attributes and capacity stays bounded.
    let mut commands = SoftRing::new(fixture.rings.command);
    let one = fixture.take(&mut commands).await.unwrap();
    let two = fixture.take(&mut commands).await.unwrap();
    assert_eq!((one.id, one.attributes), (first.raw(), default));
    assert_eq!((two.id, two.attributes), (second.raw(), pasid));
    assert!(matches!(
        client
            .submit_create_context(FunctionId::new(0), default)
            .await,
        Err(Error::Full)
    ));
}

#[tokio::test]
async fn context_completion_rejects_a_different_selector() {
    // Given: a published drain for context seven on real software rings.
    let fixture = Fixture::new().unwrap();
    let mut client = fixture.setup().await.unwrap();
    let id = client
        .submit_drain_context(
            FunctionId::new(0),
            pdma_model_contract::dma::ContextId::new(7),
        )
        .await
        .unwrap();
    let mut commands = SoftRing::new(fixture.rings.command);
    let request = fixture.take(&mut commands).await.unwrap();
    assert_eq!(request.context, 7);
    let mut responses = SoftRing::new(fixture.rings.response);
    let mut wrong = response(id.raw(), 0);
    wrong.context = 6;
    fixture.respond(&mut responses, wrong).await.unwrap();
    // When: the returned selector differs from the outstanding request.
    let result = client.try_response().await;
    // Then: correlation rejects it without returning response space.
    assert!(matches!(result, Err(Error::ManagementProtocol)));
    assert_eq!(fixture.state().unwrap().response.consumer, 0);
}

#[tokio::test]
async fn maintenance_failure_preserves_context_and_recovery_channel() {
    // Given: an abort published without waiting for firmware.
    let fixture = Fixture::new().unwrap();
    let mut client = fixture.setup().await.unwrap();
    let id = client
        .submit_abort_context(
            FunctionId::new(0),
            pdma_model_contract::dma::ContextId::new(7),
        )
        .await
        .unwrap();
    let mut commands = SoftRing::new(fixture.rings.command);
    fixture.take(&mut commands).await.unwrap();
    let mut responses = SoftRing::new(fixture.rings.response);
    let mut failed = response(id.raw(), 3);
    failed.context = 7;
    failed.status = pdma_model_contract::control::ControlResponseStatus::MaintenanceFailed;
    fixture.respond(&mut responses, failed).await.unwrap();
    // When: firmware reports the affected channel separately from the context.
    let observed = client.next_response().await.unwrap();
    // Then: the failure is collected and a subsequent request is admitted.
    assert_eq!(observed, failed);
    assert!(
        client
            .submit_reset(
                FunctionId::new(0),
                pdma_model_contract::dma::ChannelId::new(3)
            )
            .await
            .is_ok()
    );
}
