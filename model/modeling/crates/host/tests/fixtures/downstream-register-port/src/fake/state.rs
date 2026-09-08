use super::helpers::{align, lock};
use hardware_interface_runtime::{MemoryAddress, MemoryRegion};
use pdma_model_contract::semantic::{BufferId, IrqNotification};
use pdma_model_host::adapter::abi::{
    PdmaClaimError, PdmaIrqCoordinator, PdmaMappedRegion, PdmaProvisioningCause,
    PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct FakeAccess {
    pub(super) irq: PdmaIrqCoordinator,
    pub(super) state: Arc<Mutex<FakeSnapshot>>,
    pub(super) fail_allocation: Arc<AtomicBool>,
    pub(super) pending_claim: Arc<AtomicBool>,
    pub(super) pending_register: Arc<AtomicBool>,
    pub(super) next_claim_error: Arc<Mutex<Option<PdmaClaimError>>>,
    pub(super) next_register_error: Arc<Mutex<Option<PdmaRegisterAccessError>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeSnapshot {
    pub generation: u64,
    pub next_base: u64,
    pub mappings: Vec<PdmaMappedRegion>,
    pub bytes: BTreeMap<MemoryAddress, Vec<u8>>,
    pub notifications: VecDeque<IrqNotification>,
    pub registers: BTreeMap<u32, Vec<u8>>,
}

impl FakeAccess {
    pub fn new(notification: IrqNotification) -> Self {
        Self {
            irq: PdmaIrqCoordinator::default(),
            state: Arc::new(Mutex::new(FakeSnapshot {
                generation: 0,
                next_base: 0x1_0000,
                mappings: Vec::new(),
                bytes: BTreeMap::new(),
                notifications: VecDeque::from([notification]),
                registers: BTreeMap::new(),
            })),
            fail_allocation: Arc::new(AtomicBool::new(false)),
            pending_claim: Arc::new(AtomicBool::new(false)),
            pending_register: Arc::new(AtomicBool::new(false)),
            next_claim_error: Arc::new(Mutex::new(None)),
            next_register_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn snapshot(&self) -> FakeSnapshot {
        lock(&self.state).clone()
    }
    pub fn fail_next_allocation(&self) {
        self.fail_allocation.store(true, Ordering::SeqCst);
    }
    pub fn pend_next_claim(&self) {
        self.pending_claim.store(true, Ordering::SeqCst);
    }
    pub fn pend_next_register(&self) {
        self.pending_register.store(true, Ordering::SeqCst);
    }
    pub(super) fn fail_next_claim(&self, error: PdmaClaimError) {
        *lock(&self.next_claim_error) = Some(error);
    }
    pub(super) fn fail_next_register(&self, error: PdmaRegisterAccessError) {
        *lock(&self.next_register_error) = Some(error);
    }

    pub(super) fn allocate(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, PdmaProvisioningError> {
        let mut state = lock(&self.state);
        let base = align(state.next_base, request.alignment().raw())?;
        let next_base = base.checked_add(request.length().max(1)).ok_or(
            PdmaProvisioningError::Provisioning(PdmaProvisioningCause::AddressExhausted),
        )?;
        let storage = if request.length() == 0 {
            None
        } else {
            Some(
                MemoryRegion::try_new(
                    MemoryAddress::new(base),
                    request.length(),
                    request.alignment(),
                )
                .map_err(|_| PdmaProvisioningError::InvalidProjection)?,
            )
        };
        let region = PdmaMappedRegion::try_new(
            BufferId::new(state.mappings.len()),
            MemoryAddress::new(base),
            request.length(),
            request.alignment(),
            storage,
        )?;
        let bytes = vec![
            0;
            usize::try_from(request.length())
                .map_err(|_| PdmaProvisioningError::LengthOutOfRange)?
        ];
        if storage.is_some() {
            state.bytes.insert(region.base(), bytes);
        }
        state.mappings.push(region);
        state.next_base = next_base;
        state.generation += 1;
        Ok(region)
    }
}
