use hardware_interface_runtime::{
    InterruptSource, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment,
    RegisterAccess,
};
use pdma_model_host::adapter::abi::{
    AsyncDeviceAccess, PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSourceError,
    PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegionRequest,
    PdmaRegisterAccessError,
};
use std::{
    future::{Future, pending, poll_fn},
    sync::{Arc, Mutex},
    task::Poll,
};

#[derive(Debug, Default)]
struct Gate {
    write: Option<u32>,
    read: Option<u32>,
    blocked: bool,
    accesses: usize,
    fail_committed: Option<u32>,
}

#[derive(Debug, Clone)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the register gate is shared only by sibling integration test modules"
)]
pub(crate) struct GatedAccess {
    inner: AsyncDeviceAccess,
    gate: Arc<Mutex<Gate>>,
}

#[expect(
    clippy::expect_used,
    reason = "a poisoned fixture gate must fail the tests instead of hiding corrupted observations"
)]
impl GatedAccess {
    pub(crate) fn new(inner: AsyncDeviceAccess) -> Self {
        Self {
            inner,
            gate: Arc::new(Mutex::new(Gate::default())),
        }
    }

    pub(crate) fn arm_write(&self, offset: u32) {
        *self.gate.lock().expect("gate") = Gate {
            write: Some(offset),
            ..Gate::default()
        };
    }

    pub(crate) fn release(&self) {
        let mut gate = self.gate.lock().expect("gate");
        gate.write = None;
        gate.read = None;
        gate.fail_committed = None;
    }

    pub(crate) fn fail_committed(&self, offset: u32) {
        self.gate.lock().expect("gate").fail_committed = Some(offset);
    }

    pub(crate) fn is_blocked(&self) -> bool {
        self.gate.lock().expect("gate").blocked
    }

    pub(crate) fn accesses(&self) -> usize {
        self.gate.lock().expect("gate").accesses
    }
}

#[expect(
    clippy::expect_used,
    reason = "a poisoned fixture gate must fail the tests instead of hiding corrupted observations"
)]
impl RegisterAccess for GatedAccess {
    type Error = PdmaRegisterAccessError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        let blocked = {
            let mut gate = self.gate.lock().expect("gate");
            gate.accesses += 1;
            let blocked = gate.read == Some(offset);
            gate.blocked |= blocked;
            blocked
        };
        if blocked {
            pending::<()>().await;
        }
        RegisterAccess::read(&self.inner, offset, response).await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        let blocked = {
            let mut gate = self.gate.lock().expect("gate");
            gate.accesses += 1;
            let blocked = gate.write == Some(offset);
            gate.blocked |= blocked;
            blocked
        };
        if blocked {
            pending::<()>().await;
        }
        RegisterAccess::write(&self.inner, offset, request).await?;
        if self.gate.lock().expect("gate").fail_committed == Some(offset) {
            return Err(PdmaRegisterAccessError::CommittedObservation(
                hardware_interface_runtime::ObservationGateError::SequenceExhausted {
                    next_sequence: u64::MAX,
                    count: 1,
                },
            ));
        }
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

    fn at_epoch(mut self, epoch: pdma_model_contract::semantic::DeviceEpoch) -> Self {
        self.inner = self.inner.at_epoch(epoch);
        self
    }

    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        self.inner.allocate_region(request).await
    }
}

impl InterruptSource for GatedAccess {
    type Error = PdmaInterruptSourceError;
    type Notification = PdmaInterruptNotification;
    type Ready = PdmaInterruptReady;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.inner.wait_ready().await
    }

    async fn claim(
        &mut self,
        ready: Self::Ready,
    ) -> Result<Option<Self::Notification>, Self::Error> {
        self.inner.claim(ready).await
    }
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "the cancellation assertion is shared only by sibling integration test modules"
)]
pub(crate) async fn drop_pending(future: impl Future) {
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
