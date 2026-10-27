use super::{Management, ManagementRequestId, transport::ring_error};
use crate::{Error, RingConfig};
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_schema::Encode;
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::{
    control::{ControlEvent, ControlRequestOpcode, ControlRingState},
    hw_components::control::{ControlRegisters, EventRingErrorCode, EventRingErrorValues},
    memory::FunctionId,
    ring::Ring,
};
use pdma_model_software::ring::SoftRing;

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Reads firmware's retained event delivery failure without consuming an event.
    pub async fn event_error(&self) -> Result<EventRingErrorCode, Error<B::Error, M::Error>> {
        EventRingErrorValues::new(
            self.read(
                ControlRegisters::NOTIFICATION.offset()
                    + ControlNotificationRegisters::EVENT_ERROR.offset(),
            )
            .await?,
        )
        .code()
        .map_err(Error::RegisterCodec)
    }

    /// Consumes one event without completing a management request.
    /// A delivery error takes priority because event history is incomplete.
    pub async fn try_event(&mut self) -> Result<Option<ControlEvent>, Error<B::Error, M::Error>> {
        match self.event_error().await? {
            EventRingErrorCode::None => {}
            code @ (EventRingErrorCode::Overflow
            | EventRingErrorCode::AccessError
            | EventRingErrorCode::InvalidRing) => return Err(Error::EventDelivery(code)),
        }
        let io = self.layout.event_io(&self.memory);
        let event = self.event.peek(&io).await.map_err(ring_error)?;
        if event.is_some() {
            self.event.consume(&io).await.map_err(ring_error)?;
        }
        Ok(event)
    }

    /// Waits on the independent event ISR hint, not on management completions.
    pub async fn next_event(&mut self) -> Result<ControlEvent, Error<B::Error, M::Error>> {
        loop {
            if let Some(event) = self.try_event().await? {
                return Ok(event);
            }
            self.event_interrupt.wait().await;
        }
    }

    /// Requests whole-DMA recovery after PF has quiesced all affected Functions.
    /// Keep old memory alive until success; grants stay closed after recovery.
    pub async fn submit_reset_dma(
        &mut self,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit(ControlRequestOpcode::ResetDma, FunctionId::new(0), 0)
            .await
    }

    /// Reinitializes a stopped event ring and requests firmware reattachment.
    /// Retain this storage across cancellation and resume an incomplete submission.
    pub async fn reset_events(
        &mut self,
        config: RingConfig<ControlEvent>,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.require_idle()?;
        match self.event_error().await? {
            EventRingErrorCode::None => {
                return Err(Error::ManagementStatus(
                    pdma_model_contract::control::ControlResponseStatus::InvalidState,
                ));
            }
            EventRingErrorCode::Overflow
            | EventRingErrorCode::AccessError
            | EventRingErrorCode::InvalidRing => {}
        }
        let state = ControlRingState {
            base: config.base(),
            capacity: config.capacity(),
            producer: 0,
            consumer: 0,
        };
        self.memory
            .write_buffer(
                self.event_state,
                MemoryAlignment::ONE,
                &state.encode().map_err(Error::Codec)?,
            )
            .map_err(Error::Memory)?;
        self.event = SoftRing::new(config);
        self.submit(ControlRequestOpcode::ResetEvents, FunctionId::new(0), 0)
            .await
    }
}
