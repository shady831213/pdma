use super::NativeDevice;
use super::copy::{FaultRetirement, SuccessRetirement, publish_completion, publish_fault};
use crate::runner::backend::BackendStepStatus;
use pdma_model_contract::desc::BroadcastCopyCommand;
use pdma_model_contract::error::ModelResult;

pub(super) fn retire_broadcast_copy(
    device: &mut NativeDevice,
    queue_id: pdma_model_contract::QueueId,
    command: &BroadcastCopyCommand,
) -> ModelResult<BackendStepStatus> {
    let request = command.request();
    for destination in request.destinations() {
        let bytes =
            match device
                .memory
                .read_checked(request.src(), request.src_offset(), request.len())
            {
                Ok(bytes) => bytes,
                Err(source) => {
                    return publish_fault(
                        device,
                        FaultRetirement {
                            queue_id,
                            command_id: command.command_id(),
                            buffer: request.src(),
                            source,
                        },
                    );
                }
            };
        if let Err(source) =
            device
                .memory
                .write_checked(destination.dst(), destination.dst_offset(), &bytes)
        {
            return publish_fault(
                device,
                FaultRetirement {
                    queue_id,
                    command_id: command.command_id(),
                    buffer: destination.dst(),
                    source,
                },
            );
        }
    }
    publish_completion(
        device,
        SuccessRetirement {
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
        },
    )
}
