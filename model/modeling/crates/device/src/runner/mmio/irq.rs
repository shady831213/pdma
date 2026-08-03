use super::bank::{CommandBank, MmioScalar, narrow_u32};
use super::reset::details;
use crate::runner::backend::DeviceModel;
use crate::runner::irq::{
    IrqSourceView, decode_route, decode_source, encode_route, encode_source, map_mmio_error,
    observe_irq_source, usize_to_u64,
};
use crate::runner::state::RunnerState;
use pdma_model_contract::abi::mmio::{IrqCommand, MmioResult, MmioResultDetails, ResponseUpdate};
use pdma_model_contract::{IrqNotification, ModelError};

#[derive(Clone, Copy)]
pub(super) enum IrqRegister {
    ExpectedEpoch,
    SourceId,
    SourceKind,
    RouteKind,
    RouteVector,
    Command,
    Result,
    Detail0,
    Detail1,
    Detail2,
    RspEpoch,
    RspSourceId,
    RspSourceKind,
    RspState,
    RspRouteKind,
    RspNotificationKind,
    RspVector,
    RspMessageId,
    RspPendingMessages,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct IrqResponseRegisters {
    epoch: u64,
    source_id: u64,
    source_kind: u32,
    state: u32,
    route_kind: u32,
    notification_kind: u32,
    vector: u64,
    message_id: u64,
    pending_messages: u64,
}

#[derive(Debug)]
pub(super) struct IrqBank {
    expected_epoch: u64,
    source_id: u64,
    source_kind: u32,
    route_kind: u32,
    route_vector: u64,
    command: CommandBank<IrqResponseRegisters>,
}

impl IrqBank {
    pub(super) fn new() -> Self {
        Self {
            expected_epoch: 0,
            source_id: 0,
            source_kind: 0,
            route_kind: 0,
            route_vector: 0,
            command: CommandBank::new(),
        }
    }
}

pub(super) const fn read(state: &RunnerState, register: IrqRegister) -> MmioScalar {
    let bank = &state.mmio.irq;
    let response = bank.command.response;
    let result = bank.command.result;
    let [detail0, detail1, detail2] = result.details;
    match register {
        IrqRegister::ExpectedEpoch => MmioScalar::U64(bank.expected_epoch),
        IrqRegister::SourceId => MmioScalar::U64(bank.source_id),
        IrqRegister::SourceKind => MmioScalar::U32(bank.source_kind),
        IrqRegister::RouteKind => MmioScalar::U32(bank.route_kind),
        IrqRegister::RouteVector => MmioScalar::U64(bank.route_vector),
        IrqRegister::Command => MmioScalar::U32(0),
        IrqRegister::Result => MmioScalar::U32(result.result.code()),
        IrqRegister::Detail0 => MmioScalar::U64(detail0),
        IrqRegister::Detail1 => MmioScalar::U64(detail1),
        IrqRegister::Detail2 => MmioScalar::U64(detail2),
        IrqRegister::RspEpoch => MmioScalar::U64(response.epoch),
        IrqRegister::RspSourceId => MmioScalar::U64(response.source_id),
        IrqRegister::RspSourceKind => MmioScalar::U32(response.source_kind),
        IrqRegister::RspState => MmioScalar::U32(response.state),
        IrqRegister::RspRouteKind => MmioScalar::U32(response.route_kind),
        IrqRegister::RspNotificationKind => MmioScalar::U32(response.notification_kind),
        IrqRegister::RspVector => MmioScalar::U64(response.vector),
        IrqRegister::RspMessageId => MmioScalar::U64(response.message_id),
        IrqRegister::RspPendingMessages => MmioScalar::U64(response.pending_messages),
    }
}

pub(super) fn write(state: &mut RunnerState, register: IrqRegister, value: u64) {
    let bank = &mut state.mmio.irq;
    match register {
        IrqRegister::ExpectedEpoch => bank.expected_epoch = value,
        IrqRegister::SourceId => bank.source_id = value,
        IrqRegister::SourceKind => bank.source_kind = narrow_u32(value),
        IrqRegister::RouteKind => bank.route_kind = narrow_u32(value),
        IrqRegister::RouteVector => bank.route_vector = value,
        IrqRegister::Command => {
            execute(state, narrow_u32(value));
        }
        IrqRegister::Result
        | IrqRegister::Detail0
        | IrqRegister::Detail1
        | IrqRegister::Detail2
        | IrqRegister::RspEpoch
        | IrqRegister::RspSourceId
        | IrqRegister::RspSourceKind
        | IrqRegister::RspState
        | IrqRegister::RspRouteKind
        | IrqRegister::RspNotificationKind
        | IrqRegister::RspVector
        | IrqRegister::RspMessageId
        | IrqRegister::RspPendingMessages => {}
    }
}

fn execute(state: &mut RunnerState, raw_command: u32) {
    let bank = &state.mmio.irq;
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
    let (result, update) = match outcome {
        Ok(Some(response)) => (
            details(MmioResult::Ok, [0; 3]),
            ResponseUpdate::Replace(response),
        ),
        Ok(None) => (details(MmioResult::Empty, [0; 3]), ResponseUpdate::Empty),
        Err(result) => (result, ResponseUpdate::Preserve),
    };
    state.mmio.irq.command.complete(result, update);
}

fn execute_command(
    state: &mut RunnerState,
    command: IrqCommand,
    staged: (u64, u32, u64, u32, u64),
) -> Result<Option<IrqResponseRegisters>, MmioResultDetails> {
    let (epoch, source_kind, source_id, route_kind, route_vector) = staged;
    if command == IrqCommand::SnapshotLine {
        return snapshot_line(state);
    }
    if command == IrqCommand::ClaimNotification {
        return claim_notification(state);
    }
    let source = decode_source(epoch, source_kind, source_id).ok_or_else(|| {
        details(
            MmioResult::InvalidIrqSource,
            [u64::from(source_kind), source_id, 0],
        )
    })?;
    let DeviceModel::QueuePort(device) = &mut state.backend;
    let result = match command {
        IrqCommand::ConfigureRoute => decode_route(route_kind, route_vector)
            .ok_or(ModelError::DmaSubmissionState)
            .and_then(|route| device.configure_irq_route(source, route)),
        IrqCommand::Enable => device.enable_irq_source(source),
        IrqCommand::Disable => device.disable_irq_source(source),
        IrqCommand::Mask => device.mask_irq_source(source),
        IrqCommand::Unmask => device.unmask_irq_source(source),
        IrqCommand::Acknowledge => device.ack_irq_source(&state.bus_control, source),
        IrqCommand::ObserveSource => Ok(()),
        IrqCommand::SnapshotLine | IrqCommand::ClaimNotification => {
            Err(ModelError::DmaSubmissionState)
        }
    };
    result.map_err(|error| map_mmio_error(error, command, source))?;
    observe_irq_source(state, source)
        .map(source_response)
        .map(Some)
        .map_err(|error| map_mmio_error(error, command, source))
}

fn snapshot_line(state: &RunnerState) -> Result<Option<IrqResponseRegisters>, MmioResultDetails> {
    let Some(snapshot) = state.backend.irq_snapshot() else {
        return Ok(None);
    };
    let source = snapshot.cause().source_capability();
    observe_irq_source(state, source)
        .map(|view| Some(line_response(view, snapshot.cause().line().vector().raw())))
        .map_err(|error| map_mmio_error(error, IrqCommand::SnapshotLine, source))
}

fn claim_notification(
    state: &mut RunnerState,
) -> Result<Option<IrqResponseRegisters>, MmioResultDetails> {
    let notification = state.backend.take_irq_notification();
    let Some(notification) = notification else {
        return Ok(None);
    };
    let source = notification.source_capability();
    let view = observe_irq_source(state, source)
        .map_err(|error| map_mmio_error(error, IrqCommand::ClaimNotification, source))?;
    Ok(Some(match notification {
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
    }))
}

fn source_response(view: IrqSourceView) -> IrqResponseRegisters {
    let (source_kind, source_id) = encode_source(view.source.source());
    let (route_kind, vector) = encode_route(view.route);
    IrqResponseRegisters {
        epoch: view.source.epoch().raw(),
        source_id,
        source_kind,
        state: view.status.bits(),
        route_kind,
        notification_kind: 0,
        vector,
        message_id: 0,
        pending_messages: usize_to_u64(view.pending_messages),
    }
}

fn line_response(view: IrqSourceView, vector: usize) -> IrqResponseRegisters {
    IrqResponseRegisters {
        notification_kind: 1,
        vector: usize_to_u64(vector),
        pending_messages: 0,
        ..source_response(view)
    }
}
