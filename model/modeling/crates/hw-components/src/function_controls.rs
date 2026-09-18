use crate::{ControlChannel, ControlError, WindowConfigError};
use hardware_interface_schema::{RegisterAccessError, RegisterBank, StaticRegisterLayout};
use pdma_model_contract::{ControlRegisters, FunctionControlRegisters, FunctionId};

/// Independent control-register blocks selected by trusted platform Function identity.
#[derive(Debug)]
pub struct FunctionControls {
    layout: RegisterBank<StaticRegisterLayout<ControlRegisters>>,
    channels: Vec<ControlChannel>,
}

impl FunctionControls {
    /// Builds a bank without starting software or allocating ring storage.
    pub fn new(base: usize, count: usize) -> Result<Self, WindowConfigError> {
        u32::try_from(count).map_err(|_| WindowConfigError::InvalidChannelCount)?;
        let layout = FunctionControlRegisters::new(count)
            .map_err(|_| WindowConfigError::InvalidChannelCount)?
            .functions()
            .rebased(base)
            .map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(count)
            .map_err(|_| WindowConfigError::InvalidChannelCount)?;
        for index in 0..count {
            channels.push(ControlChannel::new(
                layout
                    .offset(index)
                    .ok_or(WindowConfigError::AddressUnrepresentable)?,
            ));
        }
        Ok(Self { layout, channels })
    }

    /// Reads only the selected Function's bootstrap block.
    pub fn host_read(
        &mut self,
        function: FunctionId,
        address: usize,
        bytes: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.host_channel(function, address)?
            .host_read(address, bytes)
    }

    /// Writes only the selected Function's bootstrap block.
    pub fn host_write(
        &mut self,
        function: FunctionId,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.host_channel(function, address)?
            .host_write(address, bytes)
    }

    /// Reads any control block through the privileged firmware port.
    pub fn firmware_read(
        &mut self,
        address: usize,
        bytes: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.channel(address)?.firmware_read(address, bytes)
    }

    /// Writes any response doorbell through the privileged firmware port.
    pub fn firmware_write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.channel(address)?.firmware_write(address, bytes)
    }

    /// Takes request/response pulses without losing their Function destination.
    pub fn take_notifications(&mut self, mut notify: impl FnMut(FunctionId, bool, bool)) {
        for (index, channel) in self.channels.iter_mut().enumerate() {
            if let Ok(raw) = u32::try_from(index) {
                notify(
                    FunctionId::new(raw),
                    channel.take_request_notification(),
                    channel.take_response_notification(),
                );
            }
        }
    }

    fn host_channel(
        &mut self,
        function: FunctionId,
        address: usize,
    ) -> Result<&mut ControlChannel, RegisterAccessError<ControlError>> {
        let (index, _) = self
            .layout
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        if u32::try_from(index).ok() != Some(function.raw()) {
            return Err(RegisterAccessError::Other(ControlError::Permission));
        }
        self.channel(address)
    }

    fn channel(
        &mut self,
        address: usize,
    ) -> Result<&mut ControlChannel, RegisterAccessError<ControlError>> {
        let (index, _) = self
            .layout
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        self.channels
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })
    }
}
