use super::DeviceCore;
use crate::queue::mmio::map_bus;
use pdma_model_contract::abi::mmio::QueueCommand;

impl DeviceCore {
    pub(super) fn execute_queue_command(&mut self) -> bool {
        match QueueCommand::try_from(self.queues.registers.command.raw()) {
            Ok(QueueCommand::Activate) => {
                let activation = self.queues.mmio_activation(&self.memory);
                let result = activation.and_then(|activation| {
                    self.activate_mapped_exact_queue(activation)
                        .map_err(|error| {
                            map_bus(
                                error,
                                self.queues.registers.id.raw(),
                                QueueCommand::Activate.code(),
                                0,
                            )
                        })
                });
                self.queues.complete_activation(result)
            }
            Ok(
                QueueCommand::ReadStatus
                | QueueCommand::PublishSqProducer
                | QueueCommand::CaptureSqProducer
                | QueueCommand::PublishSqConsumer
                | QueueCommand::PublishCqProducer
                | QueueCommand::PublishCqConsumer
                | QueueCommand::PublishEqProducer
                | QueueCommand::PublishEqConsumer,
            )
            | Err(_) => {
                let (changed, runnable) = self.queues.execute_mmio(self.epoch);
                if let Some(queue) = runnable {
                    self.runnable.enqueue(queue);
                }
                changed
            }
        }
    }
}
