use super::support::{Fixture, response};
use pdma_model_contract::{
    control::{ControlEvent, ControlEventKind},
    memory::FunctionId,
};
use pdma_model_software::{control::ControlQueueLayout, ring::SoftRing};

#[tokio::test]
async fn event_consumption_does_not_complete_an_outstanding_request() {
    // Given: a management request and an independently published fatal event.
    let fixture = Fixture::new().unwrap();
    let mut client = fixture.setup().await.unwrap();
    let id = client.submit_acquire(FunctionId::new(0)).await.unwrap();
    let event = ControlEvent {
        kind: ControlEventKind::DmaFatal,
        code: 1,
    };
    let layout = ControlQueueLayout::new(fixture.rings.state).unwrap();
    let mut events = SoftRing::new(fixture.rings.event);
    assert!(
        events
            .try_push(&layout.event_io(&fixture.memory), event)
            .await
            .unwrap()
    );
    // When: Host consumes the event before firmware has answered the request.
    assert_eq!(client.try_event().await.unwrap(), Some(event));
    // Then: only the event consumer advances and the original response remains collectable.
    assert_eq!(fixture.state().unwrap().event.consumer, 1);
    assert_eq!(fixture.state().unwrap().response.consumer, 0);
    assert_eq!(client.try_response().await.unwrap(), None);
    let mut responses = SoftRing::new(fixture.rings.response);
    fixture
        .respond(&mut responses, response(id.raw(), 0))
        .await
        .unwrap();
    assert_eq!(client.next_response().await.unwrap().id, id.raw());
}
