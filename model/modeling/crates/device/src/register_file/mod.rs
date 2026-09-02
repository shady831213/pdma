//! Device register-file storage behind the exact MMIO decoder.

#[doc(hidden)]
pub(crate) mod bank;
#[doc(hidden)]
pub(crate) mod global;
#[doc(hidden)]
pub(crate) mod irq;
#[doc(hidden)]
pub(crate) mod queue;

pub(crate) use bank::{CommandBank, MmioScalar, narrow_u32};
pub(crate) use global::GlobalBank;
pub(crate) use irq::IrqBank;
pub(crate) use queue::{QueueBank, QueueStaging};

#[derive(Debug)]
pub(crate) struct RegisterFile {
    pub(crate) global: GlobalBank,
    pub(crate) queue: QueueBank,
    pub(crate) irq: IrqBank,
}

impl RegisterFile {
    pub(crate) fn new() -> Self {
        Self {
            global: GlobalBank::new(),
            queue: QueueBank::new(),
            irq: IrqBank::new(),
        }
    }
}
