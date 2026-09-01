use super::{MmioRegisterError, PreparedRead, PreparedWrite, prepare_read, prepare_write};
use crate::abi::mmio::irq::support::{decode_source, irq_notification_ready};
use crate::device::DeviceCore;
use hardware_interface_runtime::{
    AccessOrigin, ObservationGate, ObservationReservation, RegisterObservation,
};
use pdma_model_contract::abi::mmio::{
    IrqCommand, MmioAccessError, MmioResult, PdmaMmioMap, RegisterId, decode_u32, decode_u64,
    decode_write,
};
use pdma_model_contract::semantic::{
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqSnapshot, IrqVector,
};

const RESPONSE_IDS: [RegisterId; 9] = [
    RegisterId::IRspEpoch,
    RegisterId::IRspSourceId,
    RegisterId::IRspSourceKind,
    RegisterId::IRspState,
    RegisterId::IRspRouteKind,
    RegisterId::IRspNotificationKind,
    RegisterId::IRspVector,
    RegisterId::IRspMessageId,
    RegisterId::IRspPendingMessages,
];

pub(crate) struct CommittedClaim {
    reservation: ObservationReservation,
    observations: Vec<RegisterObservation>,
    notification: Option<IrqNotification>,
    changed: bool,
}

impl CommittedClaim {
    pub(crate) const fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn emit(self) -> Result<Option<IrqNotification>, MmioRegisterError> {
        self.reservation.emit(self.observations)?;
        Ok(self.notification)
    }

    pub(crate) fn emit_observed(
        self,
    ) -> Result<(Option<IrqNotification>, Vec<RegisterObservation>), MmioRegisterError> {
        let observations = self.observations.clone();
        self.reservation.emit(self.observations)?;
        Ok((self.notification, observations))
    }
}

pub(crate) fn prepare_claim(
    state: &mut DeviceCore,
    register_gate: &ObservationGate,
) -> Result<CommittedClaim, MmioRegisterError> {
    let response_count = if irq_notification_ready(state) {
        RESPONSE_IDS.len()
    } else {
        0
    };
    let reservation = register_gate.reserve(2 + response_count)?;
    let sequence = reservation.start_sequence();
    let command = prepare_for_id(
        RegisterId::ICommand,
        &IrqCommand::ClaimNotification.code().to_le_bytes(),
    )?;
    let changed = command.commit(state);
    let result = read_for_id(state, RegisterId::IResult);
    let result_code = decode_prepared_u32(&result)?;
    let mut observations = Vec::with_capacity(2 + response_count);
    observations.push(command_observation(&command, sequence));
    observations.push(read_observation(&result, sequence + 1));
    let notification = if result_code == MmioResult::Empty.code() {
        None
    } else {
        let response = RESPONSE_IDS.map(|id| read_for_id(state, id));
        for (index, item) in response.iter().enumerate() {
            observations.push(read_observation(item, sequence + 2 + index as u64));
        }
        decode_notification(&response)?
    };
    Ok(CommittedClaim {
        reservation,
        observations,
        notification,
        changed,
    })
}

fn prepare_for_id(id: RegisterId, bytes: &[u8]) -> Result<PreparedWrite, MmioRegisterError> {
    let offset = PdmaMmioMap::register(id)
        .and_then(|register| u32::try_from(register.offset()).ok())
        .unwrap_or(u32::MAX);
    let (register, value) = decode_write(offset, bytes)?;
    Ok(prepare_write(register, value, offset, bytes))
}

fn read_for_id(state: &DeviceCore, id: RegisterId) -> PreparedRead {
    let offset = PdmaMmioMap::register(id)
        .and_then(|register| u32::try_from(register.offset()).ok())
        .unwrap_or(u32::MAX);
    prepare_read(state, id, offset)
}

fn command_observation(item: &PreparedWrite, sequence: u64) -> RegisterObservation {
    RegisterObservation::write(sequence, AccessOrigin::Direct, item.offset, &item.request)
}

fn read_observation(item: &PreparedRead, sequence: u64) -> RegisterObservation {
    RegisterObservation::read(sequence, AccessOrigin::Direct, item.offset, &item.response)
}

fn decode_notification(
    response: &[PreparedRead; 9],
) -> Result<Option<IrqNotification>, MmioRegisterError> {
    let epoch = decode_prepared_u64(&response[0])?;
    let source_id = decode_prepared_u64(&response[1])?;
    let source_kind = decode_prepared_u32(&response[2])?;
    let kind = decode_prepared_u32(&response[5])?;
    let vector = usize::try_from(decode_prepared_u64(&response[6])?).ok();
    let source = decode_source(epoch, source_kind, source_id);
    let (Some(vector), Some(source)) = (vector, source) else {
        return Ok(None);
    };
    Ok(match kind {
        1 => Some(IrqNotification::Line(IrqSnapshot::new(
            IrqCause::new(IrqLine::new(IrqVector::new(vector)), source),
            true,
        ))),
        2 => usize::try_from(decode_prepared_u64(&response[7])?)
            .ok()
            .map(|message_id| {
                IrqNotification::Message(IrqMessage::new(
                    IrqMessageId::new(message_id),
                    IrqVector::new(vector),
                    source,
                ))
            }),
        _ => None,
    })
}

fn decode_prepared_u32(read: &PreparedRead) -> Result<u32, MmioRegisterError> {
    let bytes = <[u8; 4]>::try_from(read.response.as_slice()).map_err(|_| wrong_width(read, 4))?;
    Ok(decode_u32(bytes))
}

fn decode_prepared_u64(read: &PreparedRead) -> Result<u64, MmioRegisterError> {
    let bytes = <[u8; 8]>::try_from(read.response.as_slice()).map_err(|_| wrong_width(read, 8))?;
    Ok(decode_u64(bytes))
}

fn wrong_width(read: &PreparedRead, expected: u128) -> MmioRegisterError {
    MmioAccessError::WrongWidth {
        offset: read.offset,
        expected,
        actual: u128::try_from(read.response.len()).unwrap_or(u128::MAX),
    }
    .into()
}
