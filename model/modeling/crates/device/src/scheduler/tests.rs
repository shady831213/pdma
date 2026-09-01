use super::Scheduler;
use pdma_model_contract::semantic::QueueId;

#[test]
fn runnable_arbitration_is_fifo_and_deduplicates_doorbells() {
    // Given: two queues and a duplicate doorbell for the first runnable queue.
    let first = QueueId::new(4);
    let second = QueueId::new(1);
    let mut scheduler = Scheduler::default();

    // When: queues are enqueued in first, second, first order.
    scheduler.enqueue(first);
    scheduler.enqueue(second);
    scheduler.enqueue(first);

    // Then: deterministic FIFO order contains each runnable queue once.
    assert_eq!(scheduler.next(), Some(first));
    assert_eq!(scheduler.next(), Some(second));
    assert_eq!(scheduler.next(), None);
}
