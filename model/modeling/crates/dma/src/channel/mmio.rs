use super::Channel;
use crate::Error;
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::dma::{
    ChannelIrqCauseValues, FaultStatusValues, ResetValues, ResumeValues, TrapAckValues,
};
use pdma_model_contract::ring::Ring;

impl Channel {
    pub(crate) fn read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let result = register_dispatch!(
            self.command.read(address, out),
            self.event.read(address, out),
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
        maintenance: bool,
    ) -> Result<(), RegisterAccessError<Error>> {
        let resetting = self.resetting();
        register_dispatch!(
            self.command
                .write(address, bytes, self.configured || maintenance, resetting),
            self.event
                .write(address, bytes, self.configured || maintenance, resetting),
            self.write_control(address, bytes, maintenance),
        )?;
        self.update_irq();
        Ok(())
    }

    fn write_control(
        &mut self,
        address: usize,
        bytes: &[u8],
        maintenance: bool,
    ) -> Result<(), RegisterAccessError<Error>> {
        let resetting = self.resetting();
        let command = &self.command;
        let locked = self.configured;
        let configured = &mut self.configured;
        self.registers.write_with(
            address,
            bytes,
            |_, old, next| {
                if resetting {
                    return Err(Error::ResetInProgress);
                }
                if maintenance
                    && (old.config.enable != next.config.enable
                        || old.config.ring_context != next.config.ring_context)
                {
                    return Err(Error::MaintenanceInProgress);
                }
                if locked && old.config.ring_context != next.config.ring_context {
                    return Err(Error::ConfigurationLocked);
                }
                if next.config.enable.raw() != 0 {
                    command
                        .validate_geometry()
                        .map_err(|_| Error::Configuration)?;
                }
                Ok(())
            },
            |_, next| {
                if next.recovery.resume.request() {
                    next.recovery.fault_status = FaultStatusValues::new(0);
                }
                next.recovery.resume = ResumeValues::new(0);
                if next.irq.trap_ack.request() {
                    next.irq.cause = ChannelIrqCauseValues::new(next.irq.cause.raw() & !2);
                }
                next.irq.trap_ack = TrapAckValues::new(0);
                if next.config.enable.raw() != 0 {
                    *configured = true;
                }
            },
        )?;
        Ok(())
    }

    pub(crate) fn write_admin(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let resetting = self.resetting();
        let mut requested = false;
        self.admin.write_with(
            address,
            bytes,
            |_, _, _| Ok(()),
            |_, next| {
                requested = next.reset.request();
                next.reset = ResetValues::new(u32::from(resetting));
            },
        )?;
        if requested {
            self.request_reset();
        }
        Ok(())
    }
}
