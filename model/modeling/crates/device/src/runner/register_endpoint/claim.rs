use super::{MmioRegisterError, PreparedRead, PreparedWrite, prepare_read, prepare_write};
use crate::runner::irq::{decode_source, irq_notification_ready};
use crate::runner::state::DeviceCore;
use hardware_interface_runtime::{AccessOrigin, ObservationReservation, RegisterObservation};
use pdma_model_contract::abi::mmio::{IrqCommand, MmioResult, PdmaMmioMap, RegisterId};
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

pub(super) struct CommittedClaim {
    reservation: ObservationReservation,
    observations: Vec<RegisterObservation>,
    notification: Option<IrqNotification>,
    generation: Option<usize>,
}

impl CommittedClaim {
    pub(super) const fn generation(&self) -> Option<usize> {
        self.generation
    }

    pub(super) fn emit(self) -> Result<Option<IrqNotification>, MmioRegisterError> {
        self.reservation.emit(self.observations)?;
        Ok(self.notification)
    }

    pub(super) fn emit_observed(
        self,
    ) -> Result<(Option<IrqNotification>, Vec<RegisterObservation>), MmioRegisterError> {
        let observations = self.observations.clone();
        self.reservation.emit(self.observations)?;
        Ok((self.notification, observations))
    }
}

impl DeviceCore {
    /// Claims one notification and synchronously emits the exact direct register transcript.
    pub fn claim_notification_exact(
        &mut self,
    ) -> Result<Option<IrqNotification>, MmioRegisterError> {
        prepare_claim(self)?.emit()
    }
}

pub(super) fn prepare_claim(state: &mut DeviceCore) -> Result<CommittedClaim, MmioRegisterError> {
    let response_count = if irq_notification_ready(state) {
        RESPONSE_IDS.len()
    } else {
        0
    };
    let reservation = state.register_gate.reserve(2 + response_count)?;
    let sequence = reservation.start_sequence();
    let previous_generation = state.generation;
    let command = prepare_for_id(
        RegisterId::ICommand,
        &IrqCommand::ClaimNotification.code().to_le_bytes(),
    );
    command.commit(state);
    let result = read_for_id(state, RegisterId::IResult);
    let result_code = decode_u32(&result.response);
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
        decode_notification(&response)
    };
    Ok(CommittedClaim {
        reservation,
        observations,
        notification,
        generation: (state.generation != previous_generation).then_some(state.generation),
    })
}

fn prepare_for_id(id: RegisterId, bytes: &[u8]) -> PreparedWrite {
    let offset = PdmaMmioMap::register(id)
        .and_then(|register| u32::try_from(register.offset()).ok())
        .unwrap_or(u32::MAX);
    prepare_write(id, decode_scalar(bytes), offset, bytes)
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

fn decode_notification(response: &[PreparedRead; 9]) -> Option<IrqNotification> {
    let epoch = decode_u64(&response[0].response);
    let source_id = decode_u64(&response[1].response);
    let source_kind = decode_u32(&response[2].response);
    let kind = decode_u32(&response[5].response);
    let vector = usize::try_from(decode_u64(&response[6].response)).ok()?;
    let source = decode_source(epoch, source_kind, source_id)?;
    match kind {
        1 => Some(IrqNotification::Line(IrqSnapshot::new(
            IrqCause::new(IrqLine::new(IrqVector::new(vector)), source),
            true,
        ))),
        2 => Some(IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(usize::try_from(decode_u64(&response[7].response)).ok()?),
            IrqVector::new(vector),
            source,
        ))),
        _ => None,
    }
}

fn decode_scalar(bytes: &[u8]) -> u64 {
    match bytes.len() {
        4 => u64::from(decode_u32(bytes)),
        8 => decode_u64(bytes),
        _ => 0,
    }
}

fn decode_u32(bytes: &[u8]) -> u32 {
    <[u8; 4]>::try_from(bytes).map_or(0, u32::from_le_bytes)
}

fn decode_u64(bytes: &[u8]) -> u64 {
    <[u8; 8]>::try_from(bytes).map_or(0, u64::from_le_bytes)
}
