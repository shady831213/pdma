use hw_model_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::hw_components::firmware::FirmwareRegisters;

/// Firmware-owned boot report with a read-only Host view.
#[derive(Debug)]
pub struct FirmwareStatus {
    registers: RegisterWindow<FirmwareRegisters>,
}

impl FirmwareStatus {
    /// Creates a Booting report with no error at the supplied base.
    pub const fn new(base: usize) -> Self {
        Self {
            registers: RegisterWindow::new(base),
        }
    }

    /// Clears a previous report during subsystem reset, not DMA/channel reset.
    pub const fn reset(&mut self) {
        self.registers = RegisterWindow::new(self.registers.base());
    }

    /// Reads the same report from either software domain.
    pub fn read(&mut self, address: usize, out: &mut [u8]) -> Result<(), RegisterAccessError> {
        self.registers.read(address, out)
    }

    /// Publishes firmware's report through its privileged bus.
    pub fn management_write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError> {
        self.registers.write(address, bytes)
    }

    /// Rejects Host writes without allowing them to change firmware state.
    pub fn host_write(&self, address: usize, _: &[u8]) -> Result<(), RegisterAccessError> {
        let offset = address
            .checked_sub(self.registers.base())
            .filter(|offset| *offset < FirmwareRegisters::EXTENT)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        Err(RegisterAccessError::WriteForbidden { offset })
    }
}
