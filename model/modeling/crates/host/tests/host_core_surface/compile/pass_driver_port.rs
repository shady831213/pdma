use pdma_model_contract::semantic::{BufferId, CompletionRecord, EpochIrqSource, EventRecord, Command, IrqNotification, IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle};
use pdma_model_host::{DriverPort, PortError};

struct DownstreamPort;

impl DriverPort for DownstreamPort {
    async fn create_queue(
        &mut self,
        _config: QueueConfig,
    ) -> Result<QueueConfigHandle, PortError> {
        Err(PortError::SubmissionState)
    }

    async fn allocate_buffer(&mut self, _len: usize) -> Result<BufferId, PortError> {
        Err(PortError::AllocationExhausted)
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn read_buffer(
        &self,
        _id: BufferId,
        _offset: usize,
        _len: usize,
    ) -> Result<Vec<u8>, PortError> {
        Ok(Vec::new())
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        _command: Command,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> Result<(), PortError> {
        Ok(())
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<CompletionRecord>, PortError> {
        Ok(None)
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<EventRecord>, PortError> {
        Ok(None)
    }

    async fn configure_irq_route(
        &mut self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn enable_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }

    async fn disable_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }

    async fn mask_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }

    async fn unmask_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }

    async fn ack_irq_source(&mut self, _source: EpochIrqSource) -> Result<(), PortError> {
        Ok(())
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        None
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        None
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        None
    }
}

fn accepts_port<P: DriverPort>(_port: P) {}

fn main() {
    accepts_port(DownstreamPort);
}
