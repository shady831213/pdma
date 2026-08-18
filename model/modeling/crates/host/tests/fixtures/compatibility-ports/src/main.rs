use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
use pdma_model_contract::semantic::{BufferId, CopyRequest, ResourceConfig, WholeDeviceReset};
use pdma_model_device::{AsyncDevice, DeviceCore};
use pdma_model_host::adapters::AsyncDeviceAccess;
use pdma_model_host::compat::{AsyncDevicePort, SemanticDevicePort};
use pdma_model_host::register::PdmaRegisterPort;
use pdma_model_host::{AsyncDriver, DriverPort, ManualDriver, ModelError};
use hardware_interface_runtime::{SoftwareCpu};
use std::{path::Path, sync::Arc};

mod compiler;
mod inventory;

fn core() -> DeviceCore {
    DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("--print-inventory") {
        if let Some(manifest) = inventory::render() {
            print!("{manifest}");
        }
        return;
    }
    let (device, worker) = AsyncDevice::from_core(core());
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let mut async_port = AsyncDevicePort::new(handle.clone());
    let cpu = SoftwareCpu::new();
    let (async_operation, shared_accounting, irq_control) = cpu
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut async_port, 1, 1).await?;
            let mut peer = driver.clone();
            let source = driver.allocate_buffer_on(&mut async_port, 1).await?;
            let destination = driver.allocate_buffer_on(&mut async_port, 1).await?;
            driver
                .write_buffer_on(&mut async_port, source, &[9])
                .await?;
            async_port
                .enable_irq_source(driver.completion_irq_source())
                .await?;
            async_port
                .unmask_irq_source(driver.completion_irq_source())
                .await?;
            driver
                .submit_copy_on(&mut async_port, CopyRequest::new(source, destination, 1))
                .await?;
            let shared = matches!(
                peer.submit_copy_on(&mut async_port, CopyRequest::new(source, destination, 1),)
                    .await,
                Err(ModelError::DriverQueueFull)
            );
            device.wait_for_progress_after(0).await;
            let completion = driver.poll_completion_on(&mut async_port).await?;
            let bytes = driver.read_buffer_on(&async_port, destination, 1).await?;
            Ok::<_, ModelError>((completion.is_some() && bytes == [9], shared, true))
        })
        .await
        .expect("async semantic operation must succeed");
    let async_store_count = usize::from(handle.memory_generation().await == 2);
    let async_fault = matches!(
        async_port.read_buffer(BufferId::new(99), 0, 1).await,
        Err(ModelError::UnknownBuffer(_))
    );
    let before_cancel = handle.memory_generation().await;
    let mut cancelled = Box::pin(async_port.allocate_buffer(1));
    let async_cancelled = tokio::select! {
        biased;
        () = std::future::ready(()) => true,
        _ = &mut cancelled => false,
    };
    drop(cancelled);
    let after_cancel = handle.memory_generation().await;
    let reset = async_port
        .reset(WholeDeviceReset::new(
            pdma_model_contract::semantic::DeviceEpoch::initial(),
        ))
        .await
        .is_ok();
    device.request_shutdown().await;
    worker.await.expect("worker must join");

    let mut sync = pdma_model_device::compat::SyncDevice::from_core(core());
    let (mut manual, destination, sync_fault) = {
        let mut sync_port = SemanticDevicePort::new(&mut sync);
        let mut manual = ManualDriver::setup_on(&mut sync_port, 1, 1)
            .await
            .expect("sync setup must succeed");
        let source = manual
            .allocate_buffer_on(&mut sync_port, 1)
            .await
            .expect("sync source allocation must succeed");
        let destination = manual
            .allocate_buffer_on(&mut sync_port, 1)
            .await
            .expect("sync destination allocation must succeed");
        manual
            .write_buffer_on(&mut sync_port, source, &[7])
            .await
            .expect("sync write must succeed");
        manual
            .submit_copy_on(&mut sync_port, CopyRequest::new(source, destination, 1))
            .await
            .expect("sync submit must succeed");
        let fault = matches!(
            sync_port.read_buffer(BufferId::new(99), 0, 1).await,
            Err(ModelError::UnknownBuffer(_))
        );
        (manual, destination, fault)
    };
    let stepped = sync.run_until_idle(8).is_ok();
    let mut sync_port = SemanticDevicePort::new(&mut sync);
    let completion = manual
        .poll_completion_on(&mut sync_port)
        .await
        .expect("sync completion poll must succeed");
    let bytes = manual
        .read_buffer_on(&sync_port, destination, 1)
        .await
        .expect("sync read must succeed");
    let sync_operation = stepped && completion.is_some() && bytes == [7];
    let sync_store_count = usize::from(sync.mapped_region_count() == 2);
    let sync_borrow_released = stepped && sync_operation;

    let (register_device, _worker) = AsyncDevice::from_core(core());
    let access = AsyncDeviceAccess::new(register_device.handle());
    let register_port = SoftwareCpu::new()
        .run_main(async { PdmaRegisterPort::new(access).is_ok() })
        .await;
    let legacy_manifest_complete =
        inventory::is_complete(include_str!("../transitional-consumers.tsv"))
            && inventory::transition_evidence_complete();
    let sync_register_rejected = compiler::rejects(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/sync_register.rs"
        )),
        "MemoryAccess",
    );
    let manual_irq_rejected = compiler::rejects(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/manual_irq.rs"
        )),
        "no method named `service_irq_on`",
    );
    let successor_paths_compile = compiler::accepts(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/positive/legacy_paths.rs"
    )));
    let retired_roots_rejected = compiler::rejects(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/retired_roots.rs"
        )),
        "unresolved import",
    );
    let new_aliases = inventory::new_alias_count();

    assert!(
        async_operation
            && sync_operation
            && async_store_count == 1
            && sync_store_count == 1
            && shared_accounting
            && reset
            && irq_control
            && sync_borrow_released
            && register_port
            && async_fault
            && async_cancelled
            && before_cancel == after_cancel
            && sync_fault
            && sync_register_rejected
            && manual_irq_rejected
            && legacy_manifest_complete
            && successor_paths_compile
            && retired_roots_rejected
            && new_aliases == 0
    );
    println!(
        "async_operation={async_operation};sync_operation={sync_operation};async_store_count={async_store_count};sync_store_count={sync_store_count};shared_accounting={shared_accounting};reset={reset};irq_control={irq_control};sync_borrow_released={sync_borrow_released};register_port={register_port}"
    );
    println!(
        "async_fault={async_fault};async_cancelled={async_cancelled};sync_fault={sync_fault};sync_register_rejected={sync_register_rejected};manual_irq_rejected={manual_irq_rejected};legacy_manifest_complete={legacy_manifest_complete};successor_paths_compile={successor_paths_compile};retired_roots_rejected={retired_roots_rejected};new_aliases={new_aliases}"
    );
}
