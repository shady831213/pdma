use super::support::{Fixture, pending_once, response};
use hardware_interface_runtime::{InterruptHandler, SoftwareCpu};
use pdma_model_host::Error;
use pdma_model_software::{control::ControlIrq, ring::SoftRing};
use std::{
    future::{Future, poll_fn},
    sync::atomic::Ordering,
    task::Poll,
};

#[tokio::test]
async fn cancelled_submission_retains_identity_and_publishes_only_once() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: the request doorbell suspends after memory publication.
            let mut client = fixture.setup().await.unwrap();
            fixture.bus.blocked.store(true, Ordering::SeqCst);
            // When: submission is cancelled and then resumed.
            pending_once(client.submit_acquire()).await;
            assert!(matches!(
                client.submit_acquire().await,
                Err(Error::ManagementBusy)
            ));
            assert_eq!(fixture.state().unwrap().command.producer, 1);
            fixture.bus.blocked.store(false, Ordering::SeqCst);
            let id = client.resume_submission().await.unwrap();
            // Then: the same request record survives, without a duplicate publication.
            let mut commands = SoftRing::new(fixture.rings.command).unwrap();
            assert_eq!(fixture.take(&mut commands).await.unwrap().id, id.raw());
            assert_eq!(fixture.state().unwrap().command.producer, 1);
            assert_eq!(client.submit_acquire().await.unwrap().raw(), id.raw() + 1);
        })
        .await;
}

#[tokio::test]
async fn cancelled_space_notification_retains_consumed_response() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: the response-space doorbell suspends after consumption.
            let mut client = fixture.setup().await.unwrap();
            let first = client.submit_acquire().await.unwrap();
            let second = client.submit_acquire().await.unwrap();
            let mut responses = SoftRing::new(fixture.rings.response).unwrap();
            fixture
                .respond(&mut responses, response(second.raw(), 1))
                .await
                .unwrap();
            fixture
                .respond(&mut responses, response(first.raw(), 0))
                .await
                .unwrap();
            fixture.bus.blocked.store(true, Ordering::SeqCst);
            // When: collection is cancelled and retried after notification can finish.
            pending_once(client.try_response()).await;
            assert_eq!(fixture.state().unwrap().response.consumer, 1);
            assert!(matches!(client.submit_acquire().await, Err(Error::Full)));
            fixture.bus.blocked.store(false, Ordering::SeqCst);
            // Then: it returns the retained completion before consuming another response.
            assert_eq!(
                client.try_response().await.unwrap(),
                Some(response(second.raw(), 1))
            );
            assert_eq!(fixture.state().unwrap().response.consumer, 1);
            assert_eq!(
                client.next_response().await.unwrap(),
                response(first.raw(), 0)
            );
            assert_eq!(fixture.state().unwrap().response.consumer, 2);
        })
        .await;
}

#[tokio::test]
async fn waiting_response_repoll_on_wrong_cpu_rejects_before_consumption() {
    let fixture = Fixture::new().unwrap();
    let owner = SoftwareCpu::new();
    let mut client = owner.run_main(fixture.setup()).await.unwrap();
    let id = owner.run_main(client.submit_acquire()).await.unwrap();
    // Given: a response wait suspended on its owning CPU.
    let mut future = Box::pin(client.next_response());
    owner
        .run_main(poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        }))
        .await;
    // When: another CPU tries to resume the same future.
    let result = SoftwareCpu::new().run_main(future).await;
    // Then: CPU ownership rejects the poll and the request remains recoverable.
    assert!(matches!(result, Err(Error::Cpu(_))));
    owner
        .run_main(async {
            let mut responses = SoftRing::new(fixture.rings.response).unwrap();
            fixture
                .respond(&mut responses, response(id.raw(), 0))
                .await
                .unwrap();
            assert_eq!(client.next_response().await.unwrap(), response(id.raw(), 0));
        })
        .await;
}

#[tokio::test]
async fn next_response_wakes_from_isr_and_wait_pending_recovers_submission() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: a cancelled submission with one real command already visible.
            let mut client = fixture.setup().await.unwrap();
            fixture.bus.blocked.store(true, Ordering::SeqCst);
            pending_once(client.submit_acquire()).await;
            fixture.bus.blocked.store(false, Ordering::SeqCst);
            let mut commands = SoftRing::new(fixture.rings.command).unwrap();
            let request = fixture.take(&mut commands).await.unwrap();
            let mut responses = SoftRing::new(fixture.rings.response).unwrap();
            // When: compatibility recovery waits, and firmware publishes then invokes the ISR.
            let mut wait = Box::pin(client.wait_pending());
            poll_fn(|cx| {
                assert!(wait.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            fixture
                .respond(&mut responses, response(request.id, 0))
                .await
                .unwrap();
            fixture.event.handler().handle(&ControlIrq).await.unwrap();
            // Then: the original request completes once through the interrupt hint.
            assert_eq!(wait.await.unwrap(), response(request.id, 0));
            assert_eq!(fixture.state().unwrap().response.consumer, 1);
            assert!(matches!(
                client.wait_pending().await,
                Err(Error::ManagementIdle)
            ));
        })
        .await;
}
