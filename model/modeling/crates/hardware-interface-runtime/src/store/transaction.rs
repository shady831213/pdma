use std::marker::PhantomData;
use std::rc::Rc;

use super::PreparedRegions;
use super::backend::MemoryState;
use super::bulk::write_store;
use crate::{
    MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryError, MemoryRegion, MemoryRevision,
};

/// An exclusive synchronous memory view confined to a transaction closure.
#[derive(Debug)]
pub struct MemoryTransaction<'store> {
    state: &'store mut MemoryState,
    not_send: PhantomData<Rc<()>>,
}

impl<'store> MemoryTransaction<'store> {
    pub(super) const fn new(state: &'store mut MemoryState) -> Self {
        Self {
            state,
            not_send: PhantomData,
        }
    }

    /// Preallocates a token branded to this exclusive view.
    pub fn prepare_regions(
        &self,
        regions: &[MemoryRegion],
    ) -> Result<TransactionPreparedRegions<'store>, MemoryError> {
        PreparedRegions::prepare(self.state, regions).map(|prepared| TransactionPreparedRegions {
            prepared,
            brand: PhantomData,
        })
    }

    /// Installs a token from this exclusive view without a fallible commit step.
    pub fn install_prepared(
        &mut self,
        prepared: TransactionPreparedRegions<'store>,
    ) -> MemoryRevision {
        let revision = prepared.prepared.next_revision;
        self.state.install(prepared.prepared);
        revision
    }

    /// Clones the exclusive view into a fallible transaction-local draft.
    pub fn try_draft(&self) -> Result<MemoryTransactionDraft<'store>, MemoryError> {
        Ok(MemoryTransactionDraft {
            state: self.state.try_clone()?,
            brand: PhantomData,
        })
    }

    /// Replaces the exclusive view with a completely validated draft.
    pub fn install_draft(&mut self, draft: MemoryTransactionDraft<'store>) -> MemoryRevision {
        let revision = draft.state.revision;
        *self.state = draft.state;
        revision
    }
}

/// A non-cloneable proposal branded to one exclusive transaction view.
#[derive(Debug)]
pub struct TransactionPreparedRegions<'store> {
    prepared: PreparedRegions,
    brand: PhantomData<&'store mut &'store ()>,
}

/// A prospective store confined to one exclusive transaction.
#[derive(Debug)]
pub struct MemoryTransactionDraft<'store> {
    state: MemoryState,
    brand: PhantomData<&'store mut &'store ()>,
}

impl MemoryTransactionDraft<'_> {
    /// Prepares and applies regions only to this prospective view.
    pub fn prepare_regions(&mut self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        let prepared = PreparedRegions::prepare(&self.state, regions)?;
        self.state.install(prepared);
        Ok(())
    }

    /// Applies one checked byte write only to this prospective view.
    pub fn write(
        &mut self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        write_store(&mut self.state, address, alignment, source)
    }
}
