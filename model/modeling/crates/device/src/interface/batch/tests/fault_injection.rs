use super::*;
use crate::DmaWrite;
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResult};
use pdma_model_contract::semantic::RingCursor;
use std::error::Error;

#[tokio::test]
async fn bootstrap_injectors_reject_before_any_prepared_install() -> TestResult {
    // Given: every typed-impossible fallible stage has a crate-private one-shot injector.
    for fault in faults() {
        let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
        let observer = Arc::new(CommitOrderObserver::default());
        let (device, _worker) = AsyncDevice::from_core_observed(
            crate::DeviceCore::new(
                Arc::clone(&memory),
                pdma_model_contract::semantic::ResourceConfig::default(),
            ),
            observer.clone(),
        );
        let handle = device.handle();
        let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
        let activation = activation(&stage)?;
        let before = handle.snapshot().await;
        let latches = queue_latches(&handle).await?;
        let trace = handle.trace_events().await;
        let wake = handle.shared.state.lock().await.wake_generation;
        handle
            .shared
            .state
            .lock()
            .await
            .core
            .batch_fault
            .set(Some(fault));

        // When: the real bootstrap reaches the selected preparation boundary.
        let result = handle
            .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
            .await;

        // Then: the injected typed failure leaves every commit surface unchanged.
        assert_injected_error(fault, result);
        assert_eq!(handle.snapshot().await, before);
        assert_eq!(handle.memory_generation().await, 0);
        assert!(memory.regions().is_empty());
        assert_eq!(queue_latches(&handle).await?, latches);
        assert_eq!(handle.trace_events().await, trace);
        assert_eq!(handle.shared.state.lock().await.wake_generation, wake);
        assert!(
            observer
                .observations
                .lock()
                .expect("probe lock available")
                .is_empty()
        );
    }
    Ok(())
}

#[tokio::test]
async fn submission_injectors_reject_before_any_prepared_install() -> TestResult {
    // Given: each typed-impossible stage is selected on an otherwise valid submission.
    for fault in faults() {
        let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
        let observer = Arc::new(CommitOrderObserver {
            memory: Some(Arc::clone(&memory)),
            ..CommitOrderObserver::default()
        });
        let (device, _worker) = AsyncDevice::from_core_observed(
            crate::DeviceCore::new(
                Arc::clone(&memory),
                pdma_model_contract::semantic::ResourceConfig::default(),
            ),
            observer.clone(),
        );
        let handle = device.handle();
        *observer.wake.lock().expect("wake probe lock available") =
            Some(handle.shared.changed.subscribe());
        let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
        let descriptor = stage
            .regions()
            .first()
            .copied()
            .ok_or("submission region required")?;
        let activation = activation(&stage)?;
        handle
            .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
            .await?;
        let indirect = handle.stage_memory_regions(&[8]).await?;
        let indirect_region = indirect
            .regions()
            .first()
            .copied()
            .ok_or("indirect region required")?;
        let before = handle.snapshot().await;
        let latches = queue_latches(&handle).await?;
        let regions = memory.regions();
        let trace = handle.trace_events().await;
        let wake = handle.shared.state.lock().await.wake_generation;
        let observations = observer
            .observations
            .lock()
            .expect("observation probe lock available")
            .len();
        {
            let core = handle.shared.state.lock().await;
            core.core.batch_fault.set(Some(fault));
        }

        // When: the real submission reaches the selected private preparation boundary.
        let result = handle
            .submit_queue_batch(super::super::SubmissionBatch::new(
                activation.queue(),
                RingCursor::new(0),
                DmaWrite::new(DmaAddress::new(descriptor.base().raw()), vec![0; 64]),
                Some((
                    indirect,
                    DmaWrite::new(DmaAddress::new(indirect_region.base().raw()), vec![0; 8]),
                )),
            ))
            .await;

        // Then: memory, mapping, queue banks, wake state, and observations remain exact.
        assert_injected_error(fault, result);
        assert_eq!(handle.snapshot().await, before);
        assert_eq!(handle.memory_generation().await, 1);
        assert_eq!(memory.regions(), regions);
        assert_eq!(queue_latches(&handle).await?, latches);
        assert_eq!(handle.trace_events().await, trace);
        assert_eq!(handle.shared.state.lock().await.wake_generation, wake);
        assert_eq!(
            observer
                .observations
                .lock()
                .expect("observation probe lock available")
                .len(),
            observations
        );
    }
    Ok(())
}

fn faults() -> [super::super::BatchPreparationFault; 6] {
    use super::super::BatchPreparationFault as F;
    [
        F::Capacity,
        F::Overlap,
        F::Response,
        F::Decode,
        F::Access,
        F::Mask,
    ]
}

fn assert_injected_error(
    fault: super::super::BatchPreparationFault,
    result: Result<QueueResponseBytes, super::super::PdmaBatchError>,
) {
    use super::super::BatchPreparationFault as F;
    match (fault, result) {
        (F::Capacity, Err(error @ super::super::PdmaBatchError::Dma(details))) => {
            assert_eq!(details.result, MmioResult::DmaStorageExhausted);
            assert!(error.source().is_none());
        }
        (F::Overlap, Err(error @ super::super::PdmaBatchError::Dma(details))) => {
            assert_eq!(details.result, MmioResult::DmaOverlap);
            assert!(error.source().is_none());
        }
        (
            F::Decode,
            Err(
                error @ super::super::PdmaBatchError::Access(MmioAccessError::ReservedOffset {
                    ..
                }),
            ),
        ) => assert_eq!(
            error.source().and_then(|source| source.downcast_ref()),
            Some(&MmioAccessError::ReservedOffset {
                offset: crate::interface::mmio::q_command_offset(),
            })
        ),
        (
            F::Access,
            Err(
                error @ super::super::PdmaBatchError::Access(MmioAccessError::WriteToReadOnly {
                    ..
                }),
            ),
        ) => assert_eq!(
            error.source().and_then(|source| source.downcast_ref()),
            Some(&MmioAccessError::WriteToReadOnly {
                offset: crate::interface::mmio::q_command_offset(),
            })
        ),
        (
            F::Mask,
            Err(
                error @ super::super::PdmaBatchError::Access(MmioAccessError::ReservedBits {
                    ..
                }),
            ),
        ) => assert_eq!(
            error.source().and_then(|source| source.downcast_ref()),
            Some(&MmioAccessError::ReservedBits {
                offset: crate::interface::mmio::q_command_offset(),
                bits: 4,
            })
        ),
        (F::Response, Err(error @ super::super::PdmaBatchError::InvalidShape)) => {
            assert!(error.source().is_none());
        }
        unexpected => panic!("unexpected injected preparation result: {unexpected:?}"),
    }
}
