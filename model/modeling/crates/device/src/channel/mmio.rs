use super::Channel;
use crate::Error;
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::Ring;

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
        register_dispatch!(
            self.command.write(address, bytes, self.configured),
            self.response.write(address, bytes, self.configured),
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
        let command = &self.command;
        let response = &self.response;
        let configured = &mut self.configured;
        self.registers.write_with(
            address,
            bytes,
            |_, _, next| {
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
            |_, next| {
                if next.reset.raw() == 0 && next.enable.raw() != 0 {
                    *configured = true;
                }
            },
        )?;
        if self.registers.values.reset.raw() != 0 {
            self.reset();
        }
        Ok(())
    }
}
