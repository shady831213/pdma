use super::support::{map_mmio_error, observe_irq_source, usize_to_u64};
use super::{CommandOutcome, line_response, source_response};
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use pdma_model_contract::abi::mmio::{IrqCommand, MmioResultDetails};
use pdma_model_contract::semantic::IrqNotification;

pub(super) fn snapshot_line(
    irq: &PdmaIrqController,
    queues: &QueueBlock,
) -> Result<CommandOutcome, MmioResultDetails> {
    let Some(snapshot) = irq.pending_snapshot() else {
        return Ok(CommandOutcome {
            response: None,
            changed: false,
        });
    };
    let source = snapshot.cause().source_capability();
    let view = observe_irq_source(irq, queues, source)
        .map_err(|error| map_mmio_error(error, IrqCommand::SnapshotLine, source))?;
    Ok(CommandOutcome {
        response: Some(line_response(view, snapshot.cause().line().vector().raw())),
        changed: false,
    })
}

pub(super) fn claim(
    irq: &mut PdmaIrqController,
    queues: &QueueBlock,
    trace: &mut TraceSink,
) -> Result<CommandOutcome, MmioResultDetails> {
    let Some(notification) = irq.take_traced_notification(trace) else {
        return Ok(CommandOutcome {
            response: None,
            changed: false,
        });
    };
    let source = notification.source_capability();
    let view = observe_irq_source(irq, queues, source)
        .map_err(|error| map_mmio_error(error, IrqCommand::ClaimNotification, source))?;
    let changed = matches!(notification, IrqNotification::Message(_));
    let response = match notification {
        IrqNotification::Line(snapshot) => {
            line_response(view, snapshot.cause().line().vector().raw())
        }
        IrqNotification::Message(message) => {
            let mut response = source_response(view);
            response.notification_kind = 2.into();
            response.vector = usize_to_u64(message.vector().raw()).into();
            response.message_id = usize_to_u64(message.id().raw()).into();
            response
        }
    };
    Ok(CommandOutcome {
        response: Some(response),
        changed,
    })
}
