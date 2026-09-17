use hardware_interface_schema::{
    RegisterAccessError, RegisterWindow, register_dispatch, register_map,
};
use pdma_model_contract::{ControlRegisters, ControlRequest, ControlResponse, Ring, Switch};

mod error;
mod ring;
pub use error::ControlError;
use ring::ControlRing;

register_map! {
    map EnableRegisters {
        alignment: 4,
        placements: [pub enable: Switch],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Domain {
    Host,
    Firmware,
}

/// Independent control-ring hardware, with no memory or allocation-policy capability.
#[derive(Debug)]
pub struct ControlChannel {
    base: usize,
    command: ControlRing<ControlRequest>,
    response: ControlRing<ControlResponse>,
    enable: RegisterWindow<EnableRegisters>,
}

impl ControlChannel {
    /// Creates disabled control queues at the supplied MMIO address.
    pub const fn new(base: usize) -> Self {
        Self {
            base,
            command: ControlRing::new(ControlRegisters::COMMAND.offset(), Domain::Host),
            response: ControlRing::new(ControlRegisters::RESPONSE.offset(), Domain::Firmware),
            enable: RegisterWindow::new(ControlRegisters::ENABLE.offset()),
        }
    }

    /// Reads control registers from the Host domain, even while disabled.
    pub fn host_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.read(address, out)
    }

    /// Reads control registers from the firmware domain, even while disabled.
    pub fn firmware_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.read(address, out)
    }

    /// Writes Host-owned configuration, command producer, or response consumer.
    pub fn host_write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.write((address, bytes), Domain::Host)
    }

    /// Writes firmware-owned command consumer or response producer.
    pub fn firmware_write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.write((address, bytes), Domain::Firmware)
    }

    fn offset(
        &self,
        address: usize,
        width: usize,
    ) -> Result<usize, RegisterAccessError<ControlError>> {
        address
            .checked_sub(self.base)
            .filter(|offset| *offset < ControlRegisters::EXTENT)
            .filter(|_| {
                width
                    .checked_sub(1)
                    .and_then(|last| address.checked_add(last))
                    .is_some()
            })
            .ok_or(RegisterAccessError::OutsideWindow { address })
    }

    fn read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        let offset = self.offset(address, out.len())?;
        register_dispatch!(
            self.command.read(offset, out),
            self.response.read(offset, out),
            self.enable
                .read(offset, out)
                .map_err(RegisterAccessError::widen)
        )
    }

    fn write(
        &mut self,
        transaction: (usize, &[u8]),
        domain: Domain,
    ) -> Result<(), RegisterAccessError<ControlError>> {
        let offset = self.offset(transaction.0, transaction.1.len())?;
        let transaction = (offset, transaction.1);
        let enabled = self.enable.values.enable.enabled();
        register_dispatch!(
            self.command.write(transaction, domain, enabled),
            self.response.write(transaction, domain, enabled),
            self.write_enable(transaction, domain)
        )
    }

    fn write_enable(
        &mut self,
        transaction: (usize, &[u8]),
        domain: Domain,
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.enable.write_with(
            transaction.0,
            transaction.1,
            |_, old, next| {
                if domain != Domain::Host {
                    return Err(ControlError::Permission);
                }
                if old.enable.enabled() && !next.enable.enabled() {
                    return Err(ControlError::ConfigurationLocked);
                }
                if next.enable.enabled() {
                    self.command
                        .validate_geometry()
                        .map_err(ControlError::Ring)?;
                    self.response
                        .validate_geometry()
                        .map_err(ControlError::Ring)?;
                }
                Ok(())
            },
            |_, _| {},
        )
    }
}
