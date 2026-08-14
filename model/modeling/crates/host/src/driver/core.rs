use super::accounting::SharedAccounting;
use crate::DriverPort;
use crate::ModelResult;
use pdma_model_contract::{
    BufferId, CompletionRecord, CompletionRingId, EventRecord, EventRouting, QueueConfig,
    QueueConfigHandle, QueueId,
};

#[derive(Debug, Clone)]
pub(super) struct DriverCore {
    pub(super) queue_config: QueueConfigHandle,
    pub(super) accounting: SharedAccounting,
}

impl DriverCore {
    pub(super) async fn setup_on(
        device: &mut impl DriverPort,
        config: QueueConfig,
        accounting: SharedAccounting,
    ) -> ModelResult<Self> {
        let queue_config = accounting.poll_valid(device.create_queue(config)).await?;
        Ok(Self {
            queue_config,
            accounting,
        })
    }

    pub(super) async fn allocate_buffer_on(
        &self,
        device: &mut impl DriverPort,
        len: usize,
    ) -> ModelResult<BufferId> {
        device.allocate_buffer(len).await
    }

    pub(super) async fn write_buffer_on(
        &self,
        device: &mut impl DriverPort,
        buffer: BufferId,
        data: &[u8],
    ) -> ModelResult<()> {
        device.write_buffer(buffer, 0, data).await
    }

    pub(super) async fn read_buffer_on(
        &self,
        device: &impl DriverPort,
        buffer: BufferId,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        device.read_buffer(buffer, 0, len).await
    }

    pub(super) async fn poll_completion_on(
        &self,
        device: &mut impl DriverPort,
    ) -> ModelResult<Option<CompletionRecord>> {
        let completion = self
            .accounting
            .poll_valid(device.poll_completion(self.queue_config))
            .await?;
        if completion.is_some() {
            self.accounting.reclaim()?;
        }
        Ok(completion)
    }

    pub(super) async fn poll_event_on(
        &self,
        device: &mut impl DriverPort,
    ) -> ModelResult<Option<EventRecord>> {
        let event = match self.queue_config.event_routing() {
            EventRouting::QueueLocal => {
                self.accounting
                    .poll_valid(device.poll_queue_event(self.queue_config))
                    .await?
            }
        };
        if event.is_some() {
            self.accounting.reclaim()?;
        }
        Ok(event)
    }

    pub(super) const fn queue_id(&self) -> QueueId {
        self.queue_config.queue_id()
    }

    pub(super) const fn completion_ring_id(&self) -> CompletionRingId {
        self.queue_config.completion_ring_id()
    }

    pub(super) const fn queue_config_handle(&self) -> QueueConfigHandle {
        self.queue_config
    }
}
