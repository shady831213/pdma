use std::marker::PhantomData;
use std::rc::Rc;

use super::bulk::write_store;
use super::{PreparedRegions, Store};
use crate::{
    MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryError, MemoryRegion, MemoryRevision,
};

/// An exclusive synchronous memory view confined to a transaction closure.
#[derive(Debug)]
pub struct MemoryTransaction<'store> {
    store: &'store mut Store,
    not_send: PhantomData<Rc<()>>,
}

impl<'store> MemoryTransaction<'store> {
    pub(super) const fn new(store: &'store mut Store) -> Self {
        Self {
            store,
            not_send: PhantomData,
        }
    }

    /// Preallocates a token branded to this exclusive view.
    pub fn prepare_regions(
        &self,
        regions: &[MemoryRegion],
    ) -> Result<TransactionPreparedRegions<'store>, MemoryError> {
        PreparedRegions::prepare(self.store, regions).map(|prepared| TransactionPreparedRegions {
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
        self.store.install(prepared.prepared);
        revision
    }

    /// Clones the exclusive view into a fallible transaction-local draft.
    pub fn try_draft(&self) -> Result<MemoryTransactionDraft<'store>, MemoryError> {
        Ok(MemoryTransactionDraft {
            store: self.store.try_clone()?,
            brand: PhantomData,
        })
    }

    /// Replaces the exclusive view with a completely validated draft.
    pub fn install_draft(&mut self, draft: MemoryTransactionDraft<'store>) -> MemoryRevision {
        let revision = draft.store.revision;
        *self.store = draft.store;
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
    store: Store,
    brand: PhantomData<&'store mut &'store ()>,
}

impl MemoryTransactionDraft<'_> {
    /// Prepares and applies regions only to this prospective view.
    pub fn prepare_regions(&mut self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        let prepared = PreparedRegions::prepare(&self.store, regions)?;
        self.store.install(prepared);
        Ok(())
    }

    /// Applies one checked byte write only to this prospective view.
    pub fn write(
        &mut self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        write_store(&mut self.store, address, alignment, source)
    }
}
