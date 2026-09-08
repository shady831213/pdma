use super::support::{RecordingObserver, TestResult, create_message, submit_copy};
use hardware_interface_runtime::{AccessOrigin, RegisterAccess};
use pdma_model_contract::abi::mmio::{MmioAccess, PdmaMmioMap};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::AsyncDevice;
use std::sync::Arc;

/// Drives the exact endpoint surface.
///
/// # Panics
/// Panics when an asserted endpoint invariant is violated.
pub(super) async fn run() -> TestResult {
    // Given: one live core, its clone-shared endpoint, and every generated register placement.
    let observer = Arc::new(RecordingObserver::default());
    let (device, worker) = AsyncDevice::new_observed(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        ResourceConfig::default(),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();

    // When: the runtime trait accesses every generated register and claims one real message.
    let resolved = PdmaMmioMap::resolve()?;
    for register in resolved.registers() {
        let offset = u32::try_from(register.offset())?;
        let definition = register.definition();
        match definition.access() {
            MmioAccess::ReadOnly | MmioAccess::ReadWrite => {
                let mut response = vec![0_u8; definition.width()];
                endpoint.read(offset, &mut response).await?;
            }
            MmioAccess::WriteOnly => {
                let request = vec![0_u8; definition.width()];
                endpoint.write(offset, &request).await?;
            }
        }
    }
    let queue = create_message(&handle).await?;
    submit_copy(&handle, queue).await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;
    let notification = super::support::claim(&endpoint).await?;

    // Then: generated coverage, exact transcript ordering, and individual consumption accesses are real.
    let observations = observer.snapshot();
    let (direct, sequential) = observations
        .split_at_checked(66)
        .ok_or("missing direct access")?;
    let ordered = observations
        .iter()
        .enumerate()
        .all(|(index, item)| usize::try_from(item.sequence) == Ok(index));
    let offsets = sequential
        .iter()
        .map(|item| format!("{:#x}/{}", item.offset, item.width))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(direct.len(), resolved.registers().len());
    assert_eq!(sequential.len(), 11);
    assert!(
        observations
            .iter()
            .all(|item| item.origin == AccessOrigin::Direct)
    );
    assert!(notification);
    assert!(ordered);
    println!(
        "registers={};direct_sequences={};sequential_items={};sequential_offsets={};origins=direct;ordered={};claim_individual_accesses=true;notification=message",
        resolved.registers().len(),
        direct.len(),
        sequential.len(),
        offsets,
        ordered,
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
