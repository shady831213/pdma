use crate::ModelError;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqResponseBank, MmioResult, MmioResultDetails, ResponseUpdate,
    apply_response_update,
};
use support::{
    IrqSourceView, decode_route, decode_source, encode_route, encode_source, map_mmio_error,
    observe_irq_source, usize_to_u64,
};

mod notification;
#[cfg(test)]
mod owner_commands;
#[doc(hidden)]
pub(crate) mod support;
#[cfg(test)]
mod tests;

impl PdmaIrqController {
    pub(crate) fn execute_mmio(&mut self, queues: &QueueBlock, trace: &mut TraceSink) -> bool {
        let raw_command = self.registers.command.raw();
        let outcome = IrqCommand::try_from(raw_command)
            .map_err(|_| details(MmioResult::UnknownCommand, [u64::from(raw_command), 0, 0]))
            .and_then(|command| self.execute_request(command, queues, trace));
        let (result, update, changed) = match outcome {
            Ok(CommandOutcome {
                response: Some(response),
                changed,
            }) => (
                details(MmioResult::Ok, [0; 3]),
                ResponseUpdate::Replace(response),
                changed,
            ),
            Ok(CommandOutcome {
                response: None,
                changed,
            }) => (
                details(MmioResult::Empty, [0; 3]),
                ResponseUpdate::Empty,
                changed,
            ),
            Err(result) => (result, ResponseUpdate::Preserve, false),
        };
        self.complete(result, update);
        changed
    }
}

impl PdmaIrqController {
    fn complete(&mut self, result: MmioResultDetails, update: ResponseUpdate<IrqResponseBank>) {
        let registers = &mut self.registers;
        registers.response = apply_response_update(registers.response, update);
        registers.result = result.result.code().into();
        let [detail0, detail1, detail2] = result.details;
        registers.detail0 = detail0.into();
        registers.detail1 = detail1.into();
        registers.detail2 = detail2.into();
    }
}

struct CommandOutcome {
    response: Option<IrqResponseBank>,
    changed: bool,
}

impl PdmaIrqController {
    fn execute_request(
        &mut self,
        command: IrqCommand,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> Result<CommandOutcome, MmioResultDetails> {
        match command {
            IrqCommand::SnapshotLine => return notification::snapshot_line(self, queues),
            IrqCommand::ClaimNotification => return notification::claim(self, queues, trace),
            IrqCommand::ConfigureRoute
            | IrqCommand::Enable
            | IrqCommand::Disable
            | IrqCommand::Mask
            | IrqCommand::Unmask
            | IrqCommand::Acknowledge
            | IrqCommand::ObserveSource => {}
        }
        let source_kind = self.registers.source_kind.raw();
        let source_id = self.registers.source_id.raw();
        let source = decode_source(self.registers.expected_epoch.raw(), source_kind, source_id)
            .ok_or_else(|| {
                details(
                    MmioResult::InvalidIrqSource,
                    [u64::from(source_kind), source_id, 0],
                )
            })?;
        let before = observe_irq_source(self, queues, source).ok();
        let result = match command {
            IrqCommand::ConfigureRoute => decode_route(
                self.registers.route_kind.raw(),
                self.registers.route_vector.raw(),
            )
            .ok_or(ModelError::DmaSubmissionState)
            .and_then(|route| self.configure_source_route(source, route, queues)),
            IrqCommand::Enable => self.enable_source(source, queues, trace),
            IrqCommand::Disable => self.disable_source(source, queues, trace),
            IrqCommand::Mask => self.mask_source(source, queues, trace),
            IrqCommand::Unmask => self.unmask_source(source, queues, trace),
            IrqCommand::Acknowledge => self.ack_source(source, queues, trace),
            IrqCommand::ObserveSource => Ok(()),
            IrqCommand::SnapshotLine | IrqCommand::ClaimNotification => {
                Err(ModelError::DmaSubmissionState)
            }
        };
        result.map_err(|error| map_mmio_error(error, command, source))?;
        let view = observe_irq_source(self, queues, source)
            .map_err(|error| map_mmio_error(error, command, source))?;
        Ok(CommandOutcome {
            response: Some(source_response(view)),
            changed: before.is_some_and(|previous| previous != view),
        })
    }
}

const fn details(result: MmioResult, details: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails { result, details }
}

fn source_response(view: IrqSourceView) -> IrqResponseBank {
    let (source_kind, source_id) = encode_source(view.source.source());
    let (route_kind, vector) = encode_route(view.route);
    IrqResponseBank {
        epoch: view.source.epoch().raw().into(),
        source_id: source_id.into(),
        source_kind: source_kind.into(),
        state: view.status.bits().into(),
        route_kind: route_kind.into(),
        notification_kind: 0.into(),
        vector: vector.into(),
        message_id: 0.into(),
        pending_messages: usize_to_u64(view.pending_messages).into(),
    }
}

fn line_response(view: IrqSourceView, vector: usize) -> IrqResponseBank {
    IrqResponseBank {
        notification_kind: 1.into(),
        vector: usize_to_u64(vector).into(),
        pending_messages: 0.into(),
        ..source_response(view)
    }
}
