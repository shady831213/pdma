use crate::memory::PdmaMemoryMappings;
use crate::queue::QueueBlock;
use crate::queue::exact::QueueStatus;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueResponseBank, ResponseUpdate, apply_response_update,
};
use pdma_model_contract::semantic::{DeviceEpoch, QueueActivation, QueueId};

mod errors;
mod queue_config;
mod queue_control;

pub(crate) use queue_control::map_bus;

#[cfg(test)]
mod tests;

impl QueueBlock {
    pub(crate) fn execute_mmio(&mut self, epoch: DeviceEpoch) -> (bool, Option<QueueId>) {
        queue_control::execute(self, epoch)
    }

    pub(crate) fn mmio_activation(
        &self,
        memory: &PdmaMemoryMappings,
    ) -> Result<QueueActivation, MmioResultDetails> {
        queue_config::activation(memory, &self.registers)
    }

    pub(crate) fn complete_activation(
        &mut self,
        result: Result<QueueStatus, MmioResultDetails>,
    ) -> bool {
        match result.and_then(status_response) {
            Ok(response) => {
                self.complete(
                    queue_control::fail(MmioResult::Ok, [0; 3]),
                    ResponseUpdate::Replace(response),
                );
                true
            }
            Err(error) => {
                self.complete(error, ResponseUpdate::Preserve);
                false
            }
        }
    }

    pub(crate) fn complete(
        &mut self,
        result: MmioResultDetails,
        update: ResponseUpdate<QueueResponseBank>,
    ) {
        let registers = &mut self.registers;
        registers.response = apply_response_update(registers.response, update);
        registers.result = result.result.code().into();
        let [detail0, detail1, detail2] = result.details;
        registers.detail0 = detail0.into();
        registers.detail1 = detail1.into();
        registers.detail2 = detail2.into();
    }
}

pub(crate) fn status_response(
    status: crate::queue::exact::QueueStatus,
) -> Result<QueueResponseBank, MmioResultDetails> {
    Ok(QueueResponseBank {
        epoch: status.queue().epoch().raw().into(),
        queue_id: u64::try_from(status.queue().queue_id().raw())
            .map_err(|_| {
                queue_control::fail(
                    pdma_model_contract::abi::mmio::MmioResult::InvalidTransaction,
                    [0; 3],
                )
            })?
            .into(),
        sq_producer: status.submission().producer().raw().into(),
        sq_captured: status.captured_submission_producer().raw().into(),
        sq_consumer: status.submission().consumer().raw().into(),
        cq_producer: status.completion().producer().raw().into(),
        cq_consumer: status.completion().consumer().raw().into(),
        eq_producer: status.event().producer().raw().into(),
        eq_consumer: status.event().consumer().raw().into(),
    })
}
