use super::support::{Fixture, response};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::{ChannelId, ControlRequestOpcode};
use pdma_model_host::Error;
use pdma_model_software::ring::SoftRing;

#[tokio::test]
async fn reset_submission_correlates_the_selected_channel() {
    // Given: a real memory-backed management queue.
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            let mut client = fixture.setup().await.unwrap();
            let mut commands = SoftRing::new(fixture.rings.command);
            let mut responses = SoftRing::new(fixture.rings.response);
            // When: Host requests reset of channel seven.
            let id = client.submit_reset(ChannelId::new(7)).await.unwrap();
            let request = fixture.take(&mut commands).await.unwrap();
            // Then: its typed opcode and channel are published, and a mismatched completion is rejected.
            assert_eq!(
                (request.id, request.opcode, request.channel),
                (id.raw(), ControlRequestOpcode::ResetChannel, 7)
            );
            fixture
                .respond(&mut responses, response(id.raw(), 6))
                .await
                .unwrap();
            assert!(matches!(
                client.try_response().await,
                Err(Error::ManagementProtocol)
            ));
        })
        .await;
}

#[tokio::test]
async fn reset_completion_preserves_request_identity() {
    // Given: a published reset for a selected channel.
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            let mut client = fixture.setup().await.unwrap();
            let mut commands = SoftRing::new(fixture.rings.command);
            let mut responses = SoftRing::new(fixture.rings.response);
            let id = client.submit_reset(ChannelId::new(2)).await.unwrap();
            fixture.take(&mut commands).await.unwrap();
            // When: firmware publishes that reset's completion.
            fixture
                .respond(&mut responses, response(id.raw(), 2))
                .await
                .unwrap();
            // Then: Host collects exactly its submitted reset and releases queue capacity.
            assert_eq!(client.next_response().await.unwrap(), response(id.raw(), 2));
            assert!(client.submit_acquire().await.is_ok());
        })
        .await;
}
