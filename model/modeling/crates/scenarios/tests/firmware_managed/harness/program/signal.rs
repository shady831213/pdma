use super::{Handler, HostVector, Ports, TestResult, buffers};
use hardware_interface_runtime::{
    AtomicReadOrder, AtomicWriteOrder, MemoryAlignment, SharedMemoryAccess, SharedMemoryAllocator,
};
use pdma_model_contract::{
    control::ControlResponseStatus,
    dma::{ChannelId, ContextId, Response},
    memory::{ContextAttributes, ContextAttributesAddressSpace, FunctionId},
};
use pdma_model_host::firmware;
use pdma_model_software::{control::InterruptEvent, signal::Signal};
use std::time::Duration;
use tokio::sync::oneshot;

/// Software-driven recovery from a pending signal Wait.
pub mod recovery;
mod work;
pub(in crate::harness) use work::run;

pub(in crate::harness) const PASID_ZERO: ContextAttributes = ContextAttributes {
    address_space: ContextAttributesAddressSpace::Pasid,
    pasid: 0,
};

#[derive(Clone, Copy, Debug)]
/// Case-selected signal values and command identifiers.
pub struct Input {
    pub(crate) initial: i64,
    pub(crate) release: i64,
    pub(crate) increment: i64,
    pub(crate) decoys: [i64; 3],
    pub(crate) ids: [u64; 4],
}

/// Software-visible completions and memory observations.
#[derive(Debug)]
pub struct Outcome {
    pub(crate) responses: [Response; 4],
    pub(crate) before_release: i64,
    pub(crate) successor_blocked: bool,
    pub(crate) final_value: i64,
    pub(crate) default_decoy: i64,
    pub(crate) same_address: bool,
}

pub(in crate::harness) struct Grant {
    channels: [ChannelId; 2],
    contexts: [ContextId; 2],
}

pub(in crate::harness) async fn manage(
    ports: Ports<'_>,
    pasid: SharedMemoryAllocator,
    input: Input,
    granted: oneshot::Sender<Grant>,
    finished: oneshot::Receiver<()>,
) -> TestResult<[i64; 2]> {
    let default_address = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
    let pasid_address = pasid.allocate(8, MemoryAlignment::try_new(8)?)?;
    let pasid_memory = pasid.memory_access();
    let default_signal = Signal::new(&ports.memory, default_address);
    let pasid_signal = Signal::new(&pasid_memory, pasid_address);
    default_signal.store(input.decoys[0], AtomicWriteOrder::Release)?;
    pasid_signal.store(input.decoys[1], AtomicWriteOrder::Release)?;
    let function = FunctionId::new(1);
    let (mut management, contexts) = configure(&ports, function).await?;
    let channels = [
        management.acquire_channel(function).await?,
        management.acquire_channel(function).await?,
    ];
    granted
        .send(Grant { channels, contexts })
        .map_err(|_| "VF stopped before grant")?;
    finished.await?;
    for channel in channels {
        management.release_channel(function, channel).await?;
    }
    for context in contexts {
        management.submit_destroy_context(function, context).await?;
        let response = management.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err(format!("context destruction failed: {:?}", response.status).into());
        }
    }
    Ok([
        default_signal.load(AtomicReadOrder::Acquire)?,
        pasid_signal.load(AtomicReadOrder::Acquire)?,
    ])
}

type Manager = pdma_model_host::Management<
    crate::harness::runtime::HostBus<SharedMemoryAccess>,
    SharedMemoryAccess,
>;

async fn configure(
    ports: &Ports<'_>,
    function: FunctionId,
) -> TestResult<(Manager, [ContextId; 2])> {
    firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        Duration::from_secs(1),
    )
    .await?;
    let interrupt = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(interrupt.handler()))?;
    let mut management = buffers::management(ports, interrupt).await?;
    let mut contexts = Vec::new();
    for attributes in [ContextAttributes::DEFAULT, PASID_ZERO] {
        management
            .submit_create_context(function, attributes)
            .await?;
        let response = management.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err(format!("context creation failed: {:?}", response.status).into());
        }
        contexts.push(ContextId::new(response.context));
    }
    Ok((
        management,
        contexts.try_into().map_err(|_| "context count")?,
    ))
}
