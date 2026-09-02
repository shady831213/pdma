use super::reset::details;
use crate::ModelError;
use crate::device::DeviceCore;
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqResponseBank, MmioResult, MmioResultDetails, ResponseUpdate,
};
use support::{
    IrqSourceView, decode_route, decode_source, encode_route, encode_source, map_mmio_error,
    observe_irq_source, usize_to_u64,
};

mod notification;
#[doc(hidden)]
pub(crate) mod support;

pub(super) fn execute(state: &mut DeviceCore, raw_command: u32) -> bool {
    let bank = &state.registers.irq;
    let staged = (
        bank.expected_epoch,
        bank.source_kind,
        bank.source_id,
        bank.route_kind,
        bank.route_vector,
    );
    let outcome = IrqCommand::try_from(raw_command)
        .map_err(|_| details(MmioResult::UnknownCommand, [u64::from(raw_command), 0, 0]))
        .and_then(|command| execute_command(state, command, staged));
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
    state.registers.irq.command.complete(result, update);
    changed
}

struct CommandOutcome {
    response: Option<IrqResponseBank>,
    changed: bool,
}

fn execute_command(
    state: &mut DeviceCore,
    command: IrqCommand,
    staged: (u64, u32, u64, u32, u64),
) -> Result<CommandOutcome, MmioResultDetails> {
    let (epoch, source_kind, source_id, route_kind, route_vector) = staged;
    if command == IrqCommand::SnapshotLine {
        return notification::snapshot_line(state);
    }
    if command == IrqCommand::ClaimNotification {
        return notification::claim(state);
    }
    let source = decode_source(epoch, source_kind, source_id).ok_or_else(|| {
        details(
            MmioResult::InvalidIrqSource,
            [u64::from(source_kind), source_id, 0],
        )
    })?;
    let before = observe_irq_source(state, source).ok();
    let result = match command {
        IrqCommand::ConfigureRoute => decode_route(route_kind, route_vector)
            .ok_or(ModelError::DmaSubmissionState)
            .and_then(|route| state.configure_irq_route(source, route)),
        IrqCommand::Enable => state.enable_irq_source(source),
        IrqCommand::Disable => state.disable_irq_source(source),
        IrqCommand::Mask => state.mask_irq_source(source),
        IrqCommand::Unmask => state.unmask_irq_source(source),
        IrqCommand::Acknowledge => state.ack_irq_source(source),
        IrqCommand::ObserveSource => Ok(()),
        IrqCommand::SnapshotLine | IrqCommand::ClaimNotification => {
            Err(ModelError::DmaSubmissionState)
        }
    };
    result.map_err(|error| map_mmio_error(error, command, source))?;
    let view = observe_irq_source(state, source)
        .map_err(|error| map_mmio_error(error, command, source))?;
    Ok(CommandOutcome {
        response: Some(source_response(view)),
        changed: before.is_some_and(|previous| previous != view),
    })
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
