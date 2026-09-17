use hardware_interface_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::{ControlRegisters, DoorbellValues};

mod error;
pub use error::ControlError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Domain {
    Host,
    Firmware,
}

/// Bootstrap and notification hardware with no ring memory or cursor state.
#[derive(Debug)]
pub struct ControlChannel {
    registers: RegisterWindow<ControlRegisters>,
    request_pending: bool,
    response_pending: bool,
}

impl ControlChannel {
    /// Creates disabled control registers at the supplied MMIO address.
    pub const fn new(base: usize) -> Self {
        Self {
            registers: RegisterWindow::new(base),
            request_pending: false,
            response_pending: false,
        }
    }

    /// Reads bootstrap or enable from the Host domain, even while disabled.
    pub fn host_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.registers
            .read(address, out)
            .map_err(RegisterAccessError::widen)
    }

    /// Reads bootstrap or enable from firmware, even while disabled.
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
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.write((address, bytes), Domain::Host)
    }

    /// Writes the firmware-owned response notification.
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

    fn write(
        &mut self,
        transaction: (usize, &[u8]),
        domain: Domain,
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.registers.write_with(
            transaction.0,
            transaction.1,
            |offset, old, next| {
                let owned = match domain {
                    Domain::Host => offset != ControlRegisters::RESPONSE.offset(),
                    Domain::Firmware => offset == ControlRegisters::RESPONSE.offset(),
                };
                if !owned {
                    return Err(ControlError::Permission);
                }
                if old.enable.enabled()
                    && (old.bootstrap != next.bootstrap || !next.enable.enabled())
                {
                    return Err(ControlError::ConfigurationLocked);
                }
                Ok(())
            },
            |_, next| {
                self.request_pending |= next.request.pulse();
                self.response_pending |= next.response.pulse();
                next.request = DoorbellValues::RESET;
                next.response = DoorbellValues::RESET;
            },
        )
    }
}
