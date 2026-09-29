use hardware_interface_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::hw_components::control::{ControlRegisters, DoorbellValues};
use pdma_model_contract::memory::{FunctionId, InvalidContextAttributes, MemoryIdentity};

mod error;
mod memory;
pub use error::ControlError;
pub use memory::{ControlMemoryAccess, MemoryError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Domain {
    Host(FunctionId),
    Firmware,
}

/// Bootstrap and notification hardware with no ring memory or cursor state.
#[derive(Debug)]
pub struct ControlChannel {
    registers: RegisterWindow<ControlRegisters>,
    manager: FunctionId,
    request_pending: bool,
    response_pending: bool,
    event_pending: bool,
}

impl ControlChannel {
    /// Returns the fixed PF and validated independent control attributes.
    pub fn memory_identity(&self) -> Result<MemoryIdentity, InvalidContextAttributes> {
        Ok(MemoryIdentity {
            function: self.manager,
            attributes: self.registers.values.attributes.attributes()?,
        })
    }
    /// Creates disabled control registers at the supplied MMIO address.
    pub const fn new(base: usize, manager: FunctionId) -> Self {
        Self {
            registers: RegisterWindow::new(base),
            manager,
            request_pending: false,
            response_pending: false,
            event_pending: false,
        }
    }

    /// Reads PF-visible configuration and event status, even while disabled.
    pub fn host_read(
        &mut self,
        function: FunctionId,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        let manager = self.manager;
        self.registers.read_with(
            address,
            out,
            |_, _| {
                if function == manager {
                    Ok(())
                } else {
                    Err(ControlError::Permission)
                }
            },
            |_, _| {},
        )
    }

    /// Reads configuration and event status from firmware, even while disabled.
    pub fn firmware_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.registers
            .read(address, out)
            .map_err(RegisterAccessError::widen)
    }

    /// Writes Host-owned bootstrap, enable, or request notification.
    pub fn host_write(
        &mut self,
        function: FunctionId,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.write((address, bytes), Domain::Host(function))
    }

    /// Writes firmware-owned notifications or event delivery status.
    pub fn firmware_write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.write((address, bytes), Domain::Firmware)
    }

    /// Takes the coalesced Host-to-firmware notification.
    pub const fn take_request_notification(&mut self) -> bool {
        core::mem::replace(&mut self.request_pending, false)
    }

    /// Takes the coalesced firmware-to-Host notification.
    pub const fn take_response_notification(&mut self) -> bool {
        core::mem::replace(&mut self.response_pending, false)
    }

    /// Takes the independent firmware-to-PF event notification.
    pub const fn take_event_notification(&mut self) -> bool {
        core::mem::replace(&mut self.event_pending, false)
    }

    fn write(
        &mut self,
        transaction: (usize, &[u8]),
        domain: Domain,
    ) -> Result<(), RegisterAccessError<ControlError>> {
        let manager = self.manager;
        self.registers.write_with(
            transaction.0,
            transaction.1,
            |offset, old, next| {
                let notification = offset >= ControlRegisters::NOTIFICATION.offset()
                    && offset
                        < ControlRegisters::NOTIFICATION.offset()
                            + ControlNotificationRegisters::EXTENT;
                let owned = match domain {
                    Domain::Host(function) => function == manager && !notification,
                    Domain::Firmware => notification,
                };
                if !owned {
                    return Err(ControlError::Permission);
                }
                if old.enable.enabled()
                    && (old.bootstrap != next.bootstrap
                        || old.attributes != next.attributes
                        || !next.enable.enabled())
                {
                    return Err(ControlError::ConfigurationLocked);
                }
                if next.enable.enabled() {
                    next.attributes
                        .attributes()
                        .map_err(|_| ControlError::InvalidAttributes)?;
                }
                Ok(())
            },
            |_, next| {
                self.request_pending |= next.request.pulse();
                self.response_pending |= next.notification.response.pulse();
                self.event_pending |= next.notification.event.pulse();
                next.request = DoorbellValues::RESET;
                next.notification.response = DoorbellValues::RESET;
                next.notification.event = DoorbellValues::RESET;
            },
        )
    }
}
