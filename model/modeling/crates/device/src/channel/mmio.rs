use super::Channel;
use crate::Error;
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::{ChannelRegisters, ResetValues, Ring};

impl Channel {
    pub(crate) fn read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let result = register_dispatch!(
            self.command.read(address, out),
            self.response.read(address, out),
            self.registers
                .read(address, out)
                .map_err(RegisterAccessError::widen),
        );
        if result.is_ok() {
            self.update_irq();
        }
        result
    }

    pub(crate) fn write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let resetting = self.resetting();
        register_dispatch!(
            self.command
                .write(address, bytes, self.configured, resetting),
            self.response
                .write(address, bytes, self.configured, resetting),
            self.write_control(address, bytes),
        )?;
        self.update_irq();
        Ok(())
    }

    fn write_control(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let resetting = self.resetting();
        let mut requested = false;
        let command = &self.command;
        let response = &self.response;
        let configured = &mut self.configured;
        self.registers.write_with(
            address,
            bytes,
            |offset, _, next| {
                if resetting && offset != ChannelRegisters::RESET.offset() {
                    return Err(Error::ResetInProgress);
                }
                if next.reset.raw() == 0 && next.enable.raw() != 0 {
                    command
                        .validate_geometry()
                        .map_err(|_| Error::Configuration)?;
                    response
                        .validate_geometry()
                        .map_err(|_| Error::Configuration)?;
                }
                Ok(())
            },
            |offset, next| {
                if offset == ChannelRegisters::RESET.offset() {
                    requested = next.reset.request();
                    next.reset = ResetValues::new(u32::from(resetting));
                } else if next.enable.raw() != 0 {
                    *configured = true;
                }
            },
        )?;
        if requested {
            self.request_reset();
        }
        Ok(())
    }
}
