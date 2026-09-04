use super::reset::details;
use crate::ModelError;
use crate::device::DeviceCore;
use crate::registers::{PdmaRegisters, narrow_u32};
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqResponseBank, MmioResult, MmioResultDetails, PdmaMmioMapViewMut, ResponseUpdate,
    apply_response_update,
};
use support::{
    IrqSourceView, decode_route, decode_source, encode_route, encode_source, map_mmio_error,
    observe_irq_source, usize_to_u64,
};

mod notification;
#[doc(hidden)]
pub(crate) mod support;
#[cfg(test)]
mod tests;

pub(super) fn execute(state: &mut DeviceCore, raw_command: u32) -> bool {
    let request = snapshot_request(&state.registers);
    let staged = (
        request.expected_epoch,
        request.source_kind,
        request.source_id,
        request.route_kind,
        request.route_vector,
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
    state
        .registers
        .view_mut(|mut root| complete(&mut root, result, update));
    changed
}

#[derive(Clone, Copy)]
struct IrqRequest {
    expected_epoch: u64,
    source_id: u64,
    source_kind: u32,
    route_kind: u32,
    route_vector: u64,
}

fn snapshot_request(registers: &PdmaRegisters) -> IrqRequest {
    registers.view(|root| {
        let irq = invariant(root.irq());
        IrqRequest {
            expected_epoch: invariant(invariant(irq.expected_epoch()).value()),
            source_id: invariant(invariant(irq.source_id()).value()),
            source_kind: narrow_u32(invariant(invariant(irq.source_kind()).value())),
            route_kind: narrow_u32(invariant(invariant(irq.route_kind()).value())),
            route_vector: invariant(invariant(irq.route_vector()).value()),
        }
    })
}

fn complete(
    root: &mut PdmaMmioMapViewMut<'_>,
    result: MmioResultDetails,
    update: ResponseUpdate<IrqResponseBank>,
) {
    let mut irq = invariant(root.irq_mut());
    let previous = IrqResponseBank {
        epoch: invariant(invariant(irq.rsp_epoch()).value()).into(),
        source_id: invariant(invariant(irq.rsp_source_id()).value()).into(),
        source_kind: narrow_u32(invariant(invariant(irq.rsp_source_kind()).value())).into(),
        state: narrow_u32(invariant(invariant(irq.rsp_state()).value())).into(),
        route_kind: narrow_u32(invariant(invariant(irq.rsp_route_kind()).value())).into(),
        notification_kind: narrow_u32(invariant(invariant(irq.rsp_notification_kind()).value()))
            .into(),
        vector: invariant(invariant(irq.rsp_vector()).value()).into(),
        message_id: invariant(invariant(irq.rsp_message_id()).value()).into(),
        pending_messages: invariant(invariant(irq.rsp_pending_messages()).value()).into(),
    };
    let response = apply_response_update(previous, update);
    invariant(invariant(irq.result_mut()).set_value(u64::from(result.result.code())));
    let [detail0, detail1, detail2] = result.details;
    invariant(invariant(irq.detail0_mut()).set_value(detail0));
    invariant(invariant(irq.detail1_mut()).set_value(detail1));
    invariant(invariant(irq.detail2_mut()).set_value(detail2));
    invariant(invariant(irq.rsp_epoch_mut()).set_value(response.epoch.raw()));
    invariant(invariant(irq.rsp_source_id_mut()).set_value(response.source_id.raw()));
    invariant(
        invariant(irq.rsp_source_kind_mut()).set_value(u64::from(response.source_kind.raw())),
    );
    invariant(invariant(irq.rsp_state_mut()).set_value(u64::from(response.state.raw())));
    invariant(invariant(irq.rsp_route_kind_mut()).set_value(u64::from(response.route_kind.raw())));
    invariant(
        invariant(irq.rsp_notification_kind_mut())
            .set_value(u64::from(response.notification_kind.raw())),
    );
    invariant(invariant(irq.rsp_vector_mut()).set_value(response.vector.raw()));
    invariant(invariant(irq.rsp_message_id_mut()).set_value(response.message_id.raw()));
    invariant(invariant(irq.rsp_pending_messages_mut()).set_value(response.pending_messages.raw()));
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

fn invariant<T, E>(result: Result<T, E>) -> T {
    let Ok(value) = result else {
        std::process::abort();
    };
    value
}
