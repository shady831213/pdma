//! Harness: provisions the platform, drives hardware/software and checks results.

pub mod fault;
pub mod hardware;
pub mod software;

use fault::{FaultBus, UnmappedRing};
use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    InterruptRuntime, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory,
    SharedMemoryAccess, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{
    ChannelFaultCode, CommandRegisters, DeviceRegisters, Response, ResponseRegisters,
    ResponseStatus,
};
use pdma_model_host::runtime::Model;
use std::{error::Error, sync::Arc};
use tokio::task::JoinSet;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

async fn run(fault_case: bool, data: &[u8]) -> TestResult<MemoryAddress> {
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let faulted = Arc::new(UnmappedRing::new(memory.clone()));
    let access = SharedMemoryAccess::from(Arc::clone(&faulted));
    let (device, mut stage) = hardware::assemble(access.clone());
    let model = Model::new(device);
    let bus = FaultBus {
        bus: model.clone(),
        base_register: if fault_case {
            Some(u32::try_from(
                DeviceRegisters::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
            )?)
        } else {
            None
        },
        memory: faulted,
    };
    let cpu = Arc::new(SoftwareCpu::new());
    let mut workers = JoinSet::new();
    let worker_model = model.clone();
    workers.spawn(async move {
        let executor = AsyncExecutor::new(worker_model);
        let mut operation = Operation::new(());
        executor.run(&mut stage, &mut operation).await
    });

    let runtime = InterruptRuntime::new(cpu, model.clone());
    assert!(
        storage.regions().is_empty(),
        "buffers must be allocated by the running software"
    );
    let (completed, summary) = runtime
        .run_main(async |interrupts| {
            let ports = software::Ports {
                bus,
                memory: access,
                allocator,
                interrupts,
            };
            software::main(ports, data).await
        })
        .await?;
    let software::Outcome {
        response,
        fault,
        destination,
    } = completed?;
    let region = storage
        .regions()
        .into_iter()
        .find(|region| region.base() == destination)
        .ok_or_else(|| std::io::Error::other("destination allocation is missing"))?;
    assert_eq!(region.alignment(), MemoryAlignment::ONE);
    let result = workers
        .join_next()
        .await
        .ok_or_else(|| std::io::Error::other("device worker is missing"))??;

    let mut copied = vec![0; data.len()];
    memory.read(destination, MemoryAlignment::ONE, &mut copied)?;
    assert_eq!(copied, data);
    assert_eq!(summary.deliveries(), 1);
    assert!(!model.irq());
    if fault_case {
        assert_eq!(fault, ChannelFaultCode::ResponseWrite);
        assert_eq!(response, None);
        assert!(result.is_err());
        println!("fault: copied={copied:?}, sticky_fault={fault:?}, IRQ handled and masked");
    } else {
        assert_eq!(
            response,
            Some(Response {
                id: 7,
                status: ResponseStatus::Success
            })
        );
        assert_eq!(fault, ChannelFaultCode::None);
        assert!(result.is_ok());
        println!("copy: copied={copied:?}, response={response:?}, IRQ handled and cleared");
    }
    Ok(destination)
}

#[tokio::test(flavor = "current_thread")]
async fn software_allocation_failure_stops_interrupt_runtime() -> TestResult {
    // Given: a platform allocator whose address space is exhausted.
    let storage = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&storage), MemoryAddress::new(u64::MAX));
    let memory = allocator.memory_access();
    let (device, _) = hardware::assemble(memory.clone());
    let model = Model::new(device);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
    // When: the software program requests its own ring storage.
    let (result, summary) = runtime
        .run_main(async |interrupts| {
            software::main(
                software::Ports {
                    bus: model.clone(),
                    memory,
                    allocator,
                    interrupts,
                },
                b"dma-copy",
            )
            .await
        })
        .await?;
    // Then: its allocation error returns without installing memory or delivering an IRQ.
    assert!(matches!(
        result.as_ref().err().and_then(|error| error
            .downcast_ref::<hardware_interface_runtime::MemoryAllocationError>(
        )),
        Some(hardware_interface_runtime::MemoryAllocationError::AddressExhausted)
    ));
    assert!(storage.regions().is_empty());
    assert_eq!(summary.deliveries(), 0);
    assert!(!model.irq());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn copy_completes_through_host_irq_delivery() -> TestResult {
    // Given: valid rings, payload memory, hardware and a software CPU.
    // When: the software submits a copy and waits for its IRQ handler.
    run(false, b"dma-copy").await?;
    // Then: the harness verified payload, success response and cleared IRQ.
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn response_write_fault_reaches_host_without_a_response_entry() -> TestResult {
    // Given: the memory endpoint will reject writes to the configured response ring.
    // When: hardware copies the payload but cannot publish its response.
    run(true, b"dma-copy").await?;
    // Then: the harness verified copied bytes, typed sticky fault and masked IRQ.
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn software_setup_failure_stops_interrupt_runtime() -> TestResult {
    // Given: the memory endpoint will reject writes to the configured command ring.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let faulted = Arc::new(UnmappedRing::new(allocator.memory_access()));
    let memory = SharedMemoryAccess::from(Arc::clone(&faulted));
    let (device, _) = hardware::assemble(memory.clone());
    let model = Model::new(device);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareCpu::new()), model.clone());
    let bus = FaultBus {
        bus: model.clone(),
        base_register: Some(u32::try_from(
            DeviceRegisters::COMMAND.offset() + CommandRegisters::BASE.offset(),
        )?),
        memory: faulted,
    };
    // When: software encounters the memory error before it registers its ISR.
    let (result, summary) = runtime
        .run_main(async |interrupts| {
            software::main(
                software::Ports {
                    bus,
                    memory,
                    allocator,
                    interrupts,
                },
                b"dma-copy",
            )
            .await
        })
        .await?;
    // Then: the original software error returns and no dispatcher waits indefinitely.
    assert!(matches!(
        result.as_ref().err().and_then(
            |error| error.downcast_ref::<pdma_model_host::Error<pdma_model_device::Error>>()
        ),
        Some(pdma_model_host::Error::Memory(_))
    ));
    assert_eq!(summary.deliveries(), 0);
    assert!(!model.irq());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn copy_accepts_a_byte_aligned_destination() -> TestResult {
    // Given: a nine-byte payload, placing the next byte-aligned allocation off an eight-byte boundary.
    let data = b"unaligned";
    // When: software allocates its buffers and submits the copy.
    let destination = run(false, data).await?;
    // Then: copying and IRQ delivery succeeded at an actually unaligned destination.
    assert_ne!(destination.raw() % 8, 0);
    Ok(())
}
