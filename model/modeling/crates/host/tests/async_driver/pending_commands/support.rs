#![expect(
    clippy::expect_used,
    reason = "test fixture assertions must fail the test"
)]

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess,
};
use pdma_model_host::adapter::abi::{
    AsyncDeviceAccess, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim, PdmaMappedRegion,
    PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError,
};
use std::{
    future::{Future, pending, poll_fn},
    sync::{Arc, Mutex},
    task::Poll,
};

#[derive(Debug, Default)]
struct Gate {
    before: Option<u32>,
    after: Option<u32>,
    response: bool,
    response_offset: u32,
    reject_publication: bool,
    blocked: bool,
    fail_read: Option<(u32, PdmaRegisterAccessError)>,
    corrupt_read: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct GatedAccess {
    inner: AsyncDeviceAccess,
    gate: Arc<Mutex<Gate>>,
}

impl GatedAccess {
    pub(crate) fn inner(&self) -> AsyncDeviceAccess {
        self.inner.clone()
    }

    pub(crate) fn fail_read(&self, offset: u32) {
        self.gate.lock().expect("gate").fail_read = Some((
            offset,
            PdmaRegisterAccessError::Access(
                pdma_model_contract::abi::mmio::MmioAccessError::UnmappedOffset { offset },
            ),
        ));
    }

    pub(crate) fn fail_observation(&self, offset: u32) {
        self.gate.lock().expect("gate").fail_read = Some((
            offset,
            PdmaRegisterAccessError::Observation(
                hardware_interface_runtime::ObservationGateError::SequenceExhausted {
                    next_sequence: u64::MAX,
                    count: 2,
                },
            ),
        ));
    }

    pub(crate) fn corrupt_read(&self, offset: u32, value: u32) {
        self.gate.lock().expect("gate").corrupt_read = Some((offset, value));
    }

    pub(crate) fn new(inner: AsyncDeviceAccess) -> Self {
        Self {
            inner,
            gate: Arc::new(Mutex::new(Gate::default())),
        }
    }

    pub(crate) fn arm_before(&self, command: u32) {
        *self.gate.lock().expect("gate") = Gate {
            before: Some(command),
            ..Gate::default()
        };
    }

    pub(crate) fn arm_after(&self, command: u32) {
        self.arm_after_at(command, 0x168);
    }

    pub(crate) fn arm_after_at(&self, command: u32, response_offset: u32) {
        *self.gate.lock().expect("gate") = Gate {
            after: Some(command),
            response_offset,
            ..Gate::default()
        };
    }

    pub(crate) fn release(&self) {
        *self.gate.lock().expect("gate") = Gate::default();
    }

    pub(crate) fn reject_publication_before_details(&self) {
        *self.gate.lock().expect("gate") = Gate {
            after: Some(3),
            response_offset: 0x170,
            reject_publication: true,
            ..Gate::default()
        };
    }

    pub(crate) fn is_blocked(&self) -> bool {
        self.gate.lock().expect("gate").blocked
    }
}

impl RegisterAccess for GatedAccess {
    type Error = PdmaRegisterAccessError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        let failure = self.gate.lock().expect("gate").fail_read;
        if let Some((_, error)) = failure.filter(|(target, _)| *target == offset) {
            return Err(error);
        }
        let blocked = {
            let mut gate = self.gate.lock().expect("gate");
            let blocked = offset == gate.response_offset && gate.response;
            gate.blocked |= blocked;
            blocked
        };
        if blocked {
            pending::<()>().await;
        }
        RegisterAccess::read(&self.inner, offset, response).await?;
        let corrupt_read = self.gate.lock().expect("gate").corrupt_read;
        if let Some((target, value)) = corrupt_read {
            if target == offset {
                response.copy_from_slice(&value.to_le_bytes());
            }
        }
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        let command = if offset == 0x164 || offset == 0x220 {
            Some(u32::from_le_bytes(
                request.try_into().expect("QCommand width"),
            ))
        } else {
            None
        };
        let blocked = {
            let mut gate = self.gate.lock().expect("gate");
            let blocked = command.is_some() && command == gate.before;
            gate.blocked |= blocked;
            blocked
        };
        if blocked {
            pending::<()>().await;
        }
        let reject = command == Some(3) && self.gate.lock().expect("gate").reject_publication;
        if reject {
            RegisterAccess::write(&self.inner, 0x110, &u64::MAX.to_le_bytes()).await?;
        }
        RegisterAccess::write(&self.inner, offset, request).await?;
        let mut gate = self.gate.lock().expect("gate");
        if command.is_some() && command == gate.after {
            gate.response = true;
        }
        drop(gate);
        Ok(())
    }
}

impl MemoryAccess for GatedAccess {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        MemoryAccess::read(&self.inner, address, alignment, bytes)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        MemoryAccess::write(&self.inner, address, alignment, bytes)
    }
}

impl PdmaMemoryProvisioning for GatedAccess {
    type Error = PdmaProvisioningError;

    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        self.inner.allocate_region(request).await
    }
}

impl PdmaExactInterruptClaim for GatedAccess {
    type Error = PdmaClaimError;

    async fn wait_interrupt_ready(&self) -> Result<bool, Self::Error> {
        self.inner.wait_interrupt_ready().await
    }

    async fn claim_interrupt_exact(&self) -> Result<PdmaClaimResult, Self::Error> {
        use pdma_model_host::adapter::abi::PdmaIrqAccess;
        self.irq_coordinator()
            .claim(self, async {
                self.inner.wait_interrupt_ready().await.expect("readiness")
            })
            .await
    }
}

impl pdma_model_host::adapter::abi::PdmaIrqAccess for GatedAccess {
    fn irq_coordinator(&self) -> &pdma_model_host::adapter::abi::PdmaIrqCoordinator {
        self.inner.irq_coordinator()
    }
}

/// # Panics
/// Panics if the first poll completes instead of reaching the configured gate.
pub async fn drop_pending(future: impl Future) {
    let mut future = Box::pin(future);
    poll_fn(|cx| {
        assert!(
            future.as_mut().poll(cx).is_pending(),
            "access must suspend at the armed boundary"
        );
        Poll::Ready(())
    })
    .await;
}
