use pdma_model_contract::{
    BufferId, CompletionRecord, CompletionRingId, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, IrqSource, ModelError, ModelResult, QueueConfig, QueueConfigHandle,
    QueueId,
};
use pdma_model_device::AsyncDeviceHandle;
use pdma_model_host::AsyncDevicePort;
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicUsize, Ordering},
};
use tokio::sync::Notify;

const READY: u8 = 0;
const BLOCK: u8 = 1;
const FAIL: u8 = 2;

#[derive(Debug, Default)]
struct OperationGate {
    mode: AtomicU8,
    inner_calls: AtomicUsize,
    entered: Notify,
    release: Notify,
}

impl OperationGate {
    fn inner_calls(&self) -> usize {
        self.inner_calls.load(Ordering::Acquire)
    }

    fn fail_next(&self) {
        self.mode.store(FAIL, Ordering::Release);
    }

    fn block(&self) {
        self.mode.store(BLOCK, Ordering::Release);
    }

    async fn wait_entered(&self) {
        self.entered.notified().await;
    }

    fn allow(&self) {
        self.mode.store(READY, Ordering::Release);
        self.release.notify_one();
    }

    async fn enter(&self) -> ModelResult<()> {
        self.entered.notify_one();
        if self.mode.load(Ordering::Acquire) == BLOCK {
            self.release.notified().await;
        }
        if self.mode.swap(READY, Ordering::AcqRel) == FAIL {
            return Err(ModelError::RingFull);
        }
        Ok(())
    }

    fn record_inner_call(&self) {
        self.inner_calls.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Default)]
struct PortControl {
    create: OperationGate,
    submit: OperationGate,
    doorbell: OperationGate,
    completion: OperationGate,
    event: OperationGate,
}

#[derive(Debug, Clone)]
pub struct BlockingPort {
    inner: AsyncDeviceHandle,
    control: Arc<PortControl>,
}

impl BlockingPort {
    pub fn new(inner: AsyncDeviceHandle) -> Self {
        Self {
            inner,
            control: Arc::new(PortControl::default()),
        }
    }

    pub fn create_calls(&self) -> usize {
        self.control.create.inner_calls()
    }

    pub fn block_create(&self) {
        self.control.create.block();
    }

    pub async fn wait_create_entered(&self) {
        self.control.create.wait_entered().await;
    }

    pub fn allow_create(&self) {
        self.control.create.allow();
    }

    pub fn fail_next_submit(&self) {
        self.control.submit.fail_next();
    }

    pub fn block_submit(&self) {
        self.control.submit.block();
    }

    pub async fn wait_submit_entered(&self) {
        self.control.submit.wait_entered().await;
    }

    pub fn allow_submit(&self) {
        self.control.submit.allow();
    }

    pub fn submit_calls(&self) -> usize {
        self.control.submit.inner_calls()
    }

    pub fn fail_next_doorbell(&self) {
        self.control.doorbell.fail_next();
    }

    pub fn block_doorbell(&self) {
        self.control.doorbell.block();
    }

    pub async fn wait_doorbell_entered(&self) {
        self.control.doorbell.wait_entered().await;
    }

    pub fn allow_doorbell(&self) {
        self.control.doorbell.allow();
    }

    pub fn doorbell_calls(&self) -> usize {
        self.control.doorbell.inner_calls()
    }

    pub fn block_completion(&self) {
        self.control.completion.block();
    }

    pub fn allow_completion(&self) {
        self.control.completion.allow();
    }

    pub fn completion_calls(&self) -> usize {
        self.control.completion.inner_calls()
    }

    pub fn block_event(&self) {
        self.control.event.block();
    }

    pub fn allow_event(&self) {
        self.control.event.allow();
    }

    pub fn event_calls(&self) -> usize {
        self.control.event.inner_calls()
    }
}

impl AsyncDevicePort for BlockingPort {
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.control.create.enter().await?;
        self.control.create.record_inner_call();
        self.inner.create_queue(config).await
    }

    async fn allocate_buffer(&mut self, len: usize) -> BufferId {
        self.inner.allocate_buffer(len).await
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.inner.write_buffer(id, offset, data).await
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        self.inner.read_buffer(id, offset, len).await
    }

    async fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.control.submit.enter().await?;
        self.control.submit.record_inner_call();
        self.inner.submit_command(queue_id, command).await
    }

    async fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        self.control.doorbell.enter().await?;
        self.control.doorbell.record_inner_call();
        self.inner.ring_doorbell(queue_id).await
    }

    async fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.control.completion.enter().await?;
        self.control.completion.record_inner_call();
        self.inner.poll_completion(completion_ring_id).await
    }

    async fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        self.control.event.enter().await?;
        self.control.event.record_inner_call();
        self.inner.poll_queue_event(queue_id).await
    }

    async fn configure_irq_route(&mut self, source: IrqSource, route: IrqRoute) -> ModelResult<()> {
        self.inner.configure_irq_route(source, route).await
    }

    async fn enable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.inner.enable_irq_source(source).await
    }

    async fn disable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.inner.disable_irq_source(source).await
    }

    async fn mask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.inner.mask_irq_source(source).await
    }

    async fn unmask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.inner.unmask_irq_source(source).await
    }

    async fn ack_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.inner.ack_irq_source(source).await
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.inner.irq_snapshot().await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        self.inner.wait_irq().await
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.inner.wait_irq_notification().await
    }
}
