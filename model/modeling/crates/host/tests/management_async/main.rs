//! Bounded management submissions use real memory rings and retained CPU-local state.

mod cancellation;
mod contexts;
mod memory_failure;
mod reset;
pub mod support;

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::Error;
use pdma_model_software::ring::SoftRing;
use support::{Fixture, response};

#[tokio::test]
async fn submits_multiple_without_firmware_and_collects_out_of_order() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: no firmware is running.
            let mut client = fixture.setup().await.unwrap();
            let mut commands = SoftRing::new(fixture.rings.command);
            let mut responses = SoftRing::new(fixture.rings.response);
            // When: two submissions complete before any firmware response exists.
            let first = client.submit_acquire(FunctionId::new(0)).await.unwrap();
            let second = client.submit_acquire(FunctionId::new(0)).await.unwrap();
            // Then: both real memory records are published with distinct IDs.
            assert_ne!(first, second);
            assert_eq!(fixture.state().unwrap().command.producer, 2);
            assert_eq!(fixture.take(&mut commands).await.unwrap().id, first.raw());
            assert_eq!(fixture.take(&mut commands).await.unwrap().id, second.raw());
            assert_eq!(client.try_response().await.unwrap(), None);
            fixture
                .respond(&mut responses, response(second.raw(), 1))
                .await
                .unwrap();
            fixture
                .respond(&mut responses, response(first.raw(), 0))
                .await
                .unwrap();
            assert_eq!(
                client.next_response().await.unwrap(),
                response(second.raw(), 1)
            );
            assert_eq!(
                client.next_response().await.unwrap(),
                response(first.raw(), 0)
            );
            assert_eq!(fixture.state().unwrap().response.consumer, 2);
        })
        .await;
}

#[tokio::test]
async fn outstanding_bound_survives_firmware_consuming_all_commands() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: two outstanding requests whose command slots firmware has freed.
            let mut client = fixture.setup().await.unwrap();
            let mut commands = SoftRing::new(fixture.rings.command);
            let first = client.submit_acquire(FunctionId::new(0)).await.unwrap();
            client.submit_acquire(FunctionId::new(0)).await.unwrap();
            fixture.take(&mut commands).await.unwrap();
            fixture.take(&mut commands).await.unwrap();
            // When: a third submission exceeds the configured bound.
            assert!(matches!(
                client.submit_acquire(FunctionId::new(0)).await,
                Err(Error::Full)
            ));
            // Then: it published nothing and did not allocate an ID.
            assert_eq!(fixture.state().unwrap().command.producer, 2);
            let mut responses = SoftRing::new(fixture.rings.response);
            fixture
                .respond(&mut responses, response(first.raw(), 0))
                .await
                .unwrap();
            client.next_response().await.unwrap();
            assert_eq!(
                client
                    .submit_acquire(FunctionId::new(0))
                    .await
                    .unwrap()
                    .raw(),
                first.raw() + 2
            );
            assert!(matches!(
                client.acquire_channel(FunctionId::new(0)).await,
                Err(Error::ManagementBusy)
            ));
        })
        .await;
}

#[tokio::test]
async fn unmatched_or_duplicate_response_is_never_consumed() {
    for duplicate in [false, true] {
        let fixture = Fixture::new().unwrap();
        SoftwareCpu::new()
            .run_main(async {
                // Given: a response with either an unknown or an already completed ID.
                let mut client = fixture.setup().await.unwrap();
                let id = client.submit_acquire(FunctionId::new(0)).await.unwrap();
                let mut responses = SoftRing::new(fixture.rings.response);
                if duplicate {
                    fixture
                        .respond(&mut responses, response(id.raw(), 0))
                        .await
                        .unwrap();
                    client.next_response().await.unwrap();
                }
                fixture
                    .respond(
                        &mut responses,
                        response(if duplicate { id.raw() } else { 99 }, 0),
                    )
                    .await
                    .unwrap();
                let consumer = fixture.state().unwrap().response.consumer;
                // When: collection checks correlation against the outstanding ledger.
                assert!(matches!(
                    client.try_response().await,
                    Err(Error::ManagementProtocol)
                ));
                // Then: the invalid entry remains unconsumed.
                assert_eq!(fixture.state().unwrap().response.consumer, consumer);
            })
            .await;
    }
}

#[tokio::test]
async fn wrong_release_channel_is_never_consumed() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: a release response that names another channel.
            let mut client = fixture.setup().await.unwrap();
            let id = client
                .submit_release(
                    FunctionId::new(0),
                    pdma_model_contract::dma::ChannelId::new(1),
                )
                .await
                .unwrap();
            let mut responses = SoftRing::new(fixture.rings.response);
            fixture
                .respond(&mut responses, response(id.raw(), 0))
                .await
                .unwrap();
            // When: collection checks the request's channel.
            assert!(matches!(
                client.try_response().await,
                Err(Error::ManagementProtocol)
            ));
            // Then: no response space was returned.
            assert_eq!(fixture.state().unwrap().response.consumer, 0);
        })
        .await;
}
