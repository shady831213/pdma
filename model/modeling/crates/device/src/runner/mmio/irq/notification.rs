use super::{CommandOutcome, line_response, source_response};
use crate::runner::irq::{map_mmio_error, observe_irq_source, usize_to_u64};
use crate::runner::state::DeviceCore;
use pdma_model_contract::abi::mmio::{IrqCommand, MmioResultDetails};
use pdma_model_contract::semantic::IrqNotification;

pub(super) fn snapshot_line(state: &DeviceCore) -> Result<CommandOutcome, MmioResultDetails> {
    let Some(snapshot) = state.backend.irq_snapshot() else {
        return Ok(CommandOutcome {
            response: None,
            changed: false,
        });
    };
    let source = snapshot.cause().source_capability();
    let view = observe_irq_source(state, source)
        .map_err(|error| map_mmio_error(error, IrqCommand::SnapshotLine, source))?;
    Ok(CommandOutcome {
        response: Some(line_response(view, snapshot.cause().line().vector().raw())),
        changed: false,
    })
}

pub(super) fn claim(state: &mut DeviceCore) -> Result<CommandOutcome, MmioResultDetails> {
    let Some(notification) = state.backend.take_irq_notification() else {
        return Ok(CommandOutcome {
            response: None,
            changed: false,
        });
    };
    let source = notification.source_capability();
    let view = observe_irq_source(state, source)
        .map_err(|error| map_mmio_error(error, IrqCommand::ClaimNotification, source))?;
    let changed = matches!(notification, IrqNotification::Message(_));
    let response = match notification {
        IrqNotification::Line(snapshot) => {
            line_response(view, snapshot.cause().line().vector().raw())
        }
        IrqNotification::Message(message) => {
            let mut response = source_response(view);
            response.notification_kind = 2;
            response.vector = usize_to_u64(message.vector().raw());
            response.message_id = usize_to_u64(message.id().raw());
            response
        }
    };
    Ok(CommandOutcome {
        response: Some(response),
        changed,
    })
}
