use super::helpers::{align, lock};
use hardware_interface_runtime::{MemoryAddress, MemoryRegion};
use pdma_model_contract::semantic::{BufferId, IrqNotification};
use pdma_model_host::adapter::abi::{
    PdmaBatchError, PdmaClaimError, PdmaMappedRegion, PdmaPreparedProposal, PdmaProvisioningCause,
    PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct FakeAccess {
    pub(super) state: Arc<Mutex<FakeSnapshot>>,
    pub(super) fail_allocation: Arc<AtomicBool>,
    pub(super) pending_claim: Arc<AtomicBool>,
    pub(super) pending_register: Arc<AtomicBool>,
    pub(super) next_batch_error: Arc<Mutex<Option<PdmaBatchError>>>,
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
    pub queue_staged: bool,
    pub indirect_staged: bool,
    pub submission_committed: bool,
    pub retirement: bool,
    pub registers: BTreeMap<u32, Vec<u8>>,
}

#[derive(Debug)]
pub struct FakeProposal {
    pub(super) generation: u64,
    next_base: u64,
    regions: Vec<PdmaMappedRegion>,
}

impl PdmaPreparedProposal for FakeProposal {
    fn regions(&self) -> &[PdmaMappedRegion] {
        &self.regions
    }
    fn generation(&self) -> u64 {
        self.generation
    }
}

impl FakeAccess {
    pub fn new(notification: IrqNotification) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeSnapshot {
                generation: 0,
                next_base: 0x1_0000,
                mappings: Vec::new(),
                bytes: BTreeMap::new(),
                notifications: VecDeque::from([notification]),
                queue_staged: false,
                indirect_staged: false,
                submission_committed: false,
                retirement: false,
                registers: BTreeMap::new(),
            })),
            fail_allocation: Arc::new(AtomicBool::new(false)),
            pending_claim: Arc::new(AtomicBool::new(false)),
            pending_register: Arc::new(AtomicBool::new(false)),
            next_batch_error: Arc::new(Mutex::new(None)),
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
    pub(super) fn fail_next_batch(&self, error: PdmaBatchError) {
        *lock(&self.next_batch_error) = Some(error);
    }
    pub(super) fn fail_next_claim(&self, error: PdmaClaimError) {
        *lock(&self.next_claim_error) = Some(error);
    }
    pub(super) fn fail_next_register(&self, error: PdmaRegisterAccessError) {
        *lock(&self.next_register_error) = Some(error);
    }

    pub(super) fn prepare(
        &self,
        requests: &[PdmaRegionRequest],
    ) -> Result<FakeProposal, PdmaProvisioningError> {
        let state = lock(&self.state);
        let mut base = state.next_base;
        let mut regions = Vec::with_capacity(requests.len());
        for (index, request) in requests.iter().copied().enumerate() {
            base = align(base, request.alignment().raw())?;
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
            regions.push(PdmaMappedRegion::try_new(
                BufferId::new(state.mappings.len() + index),
                MemoryAddress::new(base),
                request.length(),
                request.alignment(),
                storage,
            )?);
            base = base.checked_add(request.length().max(1)).ok_or(
                PdmaProvisioningError::Provisioning(PdmaProvisioningCause::AddressExhausted),
            )?;
        }
        Ok(FakeProposal {
            generation: state.generation,
            next_base: base,
            regions,
        })
    }

    pub(super) fn commit(
        &self,
        proposal: FakeProposal,
    ) -> Result<Vec<PdmaMappedRegion>, PdmaProvisioningError> {
        let mut state = lock(&self.state);
        if proposal.generation != state.generation {
            return Err(PdmaProvisioningError::Provisioning(
                PdmaProvisioningCause::StaleProposal {
                    expected: proposal.generation,
                    actual: state.generation,
                },
            ));
        }
        for region in &proposal.regions {
            if region.length() != 0 {
                state.bytes.insert(
                    region.base(),
                    vec![
                        0;
                        usize::try_from(region.length())
                            .map_err(|_| PdmaProvisioningError::LengthOutOfRange)?
                    ],
                );
            }
        }
        state.mappings.extend(proposal.regions.iter().copied());
        state.next_base = proposal.next_base;
        state.generation += 1;
        Ok(proposal.regions)
    }
}
