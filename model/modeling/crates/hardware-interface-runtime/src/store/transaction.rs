use std::marker::PhantomData;
use std::rc::Rc;

use super::{PreparedRegions, Store};
use crate::{MemoryError, MemoryRegion, MemoryRevision};

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
}

/// A non-cloneable proposal branded to one exclusive transaction view.
#[derive(Debug)]
pub struct TransactionPreparedRegions<'store> {
    prepared: PreparedRegions,
    brand: PhantomData<&'store mut &'store ()>,
}
