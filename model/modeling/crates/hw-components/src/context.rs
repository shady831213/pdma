use crate::windows::ChannelWindows;
use hardware_interface_schema::{
    RegisterAccessError, RegisterBank, RegisterBankError, RegisterShape, RegisterWindow,
    StaticRegisterLayout, register_dispatch,
};
use pdma_model_contract::dma::ContextId;
use pdma_model_contract::hw_components::context::{
    ChannelFailures, ContextEntryRegisters, ContextManagementRegisters, ContextTableRegisters,
};
use pdma_model_contract::register::ReadWordValues;
use pdma_model_dma::Dma;

mod error;
mod maintenance;
mod memory;
pub use error::ContextConfigError;
pub use memory::{ContextMemoryAccess, MemoryError};

#[doc = "Firmware-owned context bindings and hardware maintenance completion."]
#[derive(Debug)]
pub struct ContextTable {
    entries: Vec<RegisterWindow<ContextEntryRegisters>>,
    bank: RegisterBank<StaticRegisterLayout<ContextEntryRegisters>>,
    management: RegisterWindow<ContextManagementRegisters>,
    pending: Option<ContextId>,
    failed: Vec<RegisterWindow<ChannelFailures>>,
    failed_bank: RegisterBank<StaticRegisterLayout<ChannelFailures>>,
    aperture: core::ops::Range<usize>,
}

impl ContextTable {
    #[doc = "Creates an instance-sized table with no allocated entries."]
    ///
    /// # Errors
    /// Returns a capacity, layout, or allocation error when construction fails.
    pub fn new(
        count: usize,
        channel_count: usize,
        base: usize,
    ) -> Result<Self, ContextConfigError> {
        let count_word = u32::try_from(count).map_err(|_| ContextConfigError::InvalidCapacity)?;
        let layout = ContextTableRegisters::for_channels(count, channel_count)
            .map_err(ContextConfigError::Layout)?;
        if base % layout.alignment() != 0 {
            return Err(ContextConfigError::Layout(RegisterBankError::Alignment));
        }
        let end = base
            .checked_add(layout.extent())
            .ok_or(ContextConfigError::Layout(RegisterBankError::Overflow))?;
        let bank = layout
            .entries()
            .rebased(base)
            .map_err(ContextConfigError::Layout)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(ContextConfigError::Allocation)?;
        for index in 0..count {
            entries
                .push(RegisterWindow::new(bank.offset(index).ok_or(
                    ContextConfigError::Layout(RegisterBankError::Overflow),
                )?));
        }
        let mut management = RegisterWindow::<ContextManagementRegisters>::new(
            base + ContextTableRegisters::MANAGEMENT.offset(),
        );
        management.values.capacity = ReadWordValues::new(count_word);
        let failed_bank = layout
            .failed()
            .rebased(base)
            .map_err(ContextConfigError::Layout)?;
        let mut failed = Vec::new();
        failed
            .try_reserve_exact(failed_bank.count())
            .map_err(ContextConfigError::Allocation)?;
        for index in 0..failed_bank.count() {
            failed.push(RegisterWindow::new(
                failed_bank
                    .offset(index)
                    .ok_or(ContextConfigError::Layout(RegisterBankError::Overflow))?,
            ));
        }
        Ok(Self {
            entries,
            bank,
            management,
            pending: None,
            failed,
            failed_bank,
            aperture: base..end,
        })
    }

    #[doc = "Reads the generated firmware management aperture."]
    pub fn management_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError> {
        register_dispatch!(
            self.management.read(address, out),
            self.read_failed(address, out),
            self.entry(address)?.read(address, out)
        )
    }

    #[doc = "Denies Host reads to this firmware-only aperture."]
    pub fn host_read(&self, address: usize, _: &mut [u8]) -> Result<(), RegisterAccessError> {
        if self.aperture.contains(&address) {
            Err(RegisterAccessError::ReadForbidden {
                offset: address - self.aperture.start,
            })
        } else {
            Err(RegisterAccessError::OutsideWindow { address })
        }
    }

    #[doc = "Denies Host writes to this firmware-only aperture."]
    pub fn host_write(&self, address: usize, _: &[u8]) -> Result<(), RegisterAccessError> {
        if self.aperture.contains(&address) {
            Err(RegisterAccessError::WriteForbidden {
                offset: address - self.aperture.start,
            })
        } else {
            Err(RegisterAccessError::OutsideWindow { address })
        }
    }

    #[doc = "Publishes entries or starts maintenance against the actual DMA engine and owner bank."]
    pub fn management_write<M, S>(
        &mut self,
        transaction: (usize, &[u8]),
        hardware: (&mut Dma<M, S>, &ChannelWindows),
    ) -> Result<(), RegisterAccessError> {
        register_dispatch!(
            self.write_management(transaction, hardware),
            self.write_entry(transaction),
            self.write_failed(transaction)
        )
    }

    fn entry(
        &mut self,
        address: usize,
    ) -> Result<&mut RegisterWindow<ContextEntryRegisters>, RegisterAccessError> {
        let (index, _) = self
            .bank
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        self.entries
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })
    }

    fn write_entry(&mut self, transaction: (usize, &[u8])) -> Result<(), RegisterAccessError> {
        let pending = self.pending.is_some();
        let entry = self.entry(transaction.0)?;
        if entry.values.valid.enabled()
            && (pending || transaction.0 != entry.base() + ContextEntryRegisters::VALID.offset())
        {
            return Err(RegisterAccessError::WriteForbidden {
                offset: transaction.0 - entry.base(),
            });
        }
        let mut next = RegisterWindow::<ContextEntryRegisters>::new(entry.base());
        next.values = entry.values;
        next.write(transaction.0, transaction.1)?;
        if next.values.valid.enabled() && next.values.attributes.attributes().is_err() {
            return Err(RegisterAccessError::WriteForbidden {
                offset: transaction.0 - entry.base(),
            });
        }
        entry.values = next.values;
        Ok(())
    }

    fn read_failed(&mut self, address: usize, out: &mut [u8]) -> Result<(), RegisterAccessError> {
        let (index, _) = self
            .failed_bank
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        self.failed
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })?
            .read(address, out)
    }

    fn write_failed(
        &mut self,
        (address, bytes): (usize, &[u8]),
    ) -> Result<(), RegisterAccessError> {
        let (index, _) = self
            .failed_bank
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        self.failed
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })?
            .write(address, bytes)
    }
}
