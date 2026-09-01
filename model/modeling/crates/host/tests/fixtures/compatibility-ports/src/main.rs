use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::{BufferId, CopyRequest, ResourceConfig, WholeDeviceReset};
use pdma_model_device::{AsyncDevice, DeviceCore};
use pdma_model_host::adapter::abi::AsyncDeviceAccess;
use pdma_model_host::compat::AsyncDevicePort;
use pdma_model_host::adapter::abi::PdmaRegisterPort;
use pdma_model_host::{AsyncDriver, DriverError, DriverPort, PortError};
use std::{path::Path, sync::Arc};

mod compiler;

fn core() -> DeviceCore {
    DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
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
                Err(DriverError::QueueFull)
            );
            device.wait_for_progress_after(0).await;
            let completion = driver.poll_completion_on(&mut async_port).await?;
            let bytes = driver.read_buffer_on(&async_port, destination, 1).await?;
            Ok::<_, DriverError>((completion.is_some() && bytes == [9], shared, true))
        })
        .await
        .expect("async semantic operation must succeed");
    let async_store_count = usize::from(handle.memory_generation().await == 2);
    let async_fault = matches!(
        async_port.read_buffer(BufferId::new(99), 0, 1).await,
        Err(PortError::UnknownBuffer(_))
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

    let (register_device, _worker) = AsyncDevice::from_core(core());
    let access = AsyncDeviceAccess::new(register_device.handle());
    let register_port = SoftwareCpu::new()
        .run_main(async { PdmaRegisterPort::new(access).is_ok() })
        .await;
    let manual_irq_rejected = compiler::rejects(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/manual_irq.rs"
        )),
        "no method named `service_irq_on`",
    );
    let successor_paths_compile = compiler::accepts(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/positive/canonical_paths.rs"
    )));
    let final_error_names_compile = compiler::accepts(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/positive/final_error_names.rs"
    )));
    let transitional_error_aliases_rejected = compiler::rejects_all(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/transitional_error_aliases.rs"
        )),
        &["unresolved imports", "BusControlError", "MmioBatchError"],
    );
    let retired_roots_rejected = compiler::rejects(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/retired_roots.rs"
        )),
        "unresolved import",
    );
    let raw_devices_rejected = compiler::rejects(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/negative/raw_devices.rs"
        )),
        "the trait bound",
    );
    assert!(
        async_operation
            && async_store_count == 1
            && shared_accounting
            && reset
            && irq_control
            && register_port
            && async_fault
            && async_cancelled
            && before_cancel == after_cancel
            && manual_irq_rejected
            && successor_paths_compile
            && final_error_names_compile
            && transitional_error_aliases_rejected
            && retired_roots_rejected
            && raw_devices_rejected,
        "async_operation={async_operation};async_store_count={async_store_count};shared_accounting={shared_accounting};reset={reset};irq_control={irq_control};register_port={register_port};async_fault={async_fault};async_cancelled={async_cancelled};cancel_unchanged={};manual_irq_rejected={manual_irq_rejected};successor_paths_compile={successor_paths_compile};final_error_names_compile={final_error_names_compile};transitional_error_aliases_rejected={transitional_error_aliases_rejected};retired_roots_rejected={retired_roots_rejected};raw_devices_rejected={raw_devices_rejected}",
        before_cancel == after_cancel
    );
    println!(
        "async_operation={async_operation};async_store_count={async_store_count};shared_accounting={shared_accounting};reset={reset};irq_control={irq_control};register_port={register_port}"
    );
    println!(
        "async_fault={async_fault};async_cancelled={async_cancelled};manual_irq_rejected={manual_irq_rejected};successor_paths_compile={successor_paths_compile};final_error_names_compile={final_error_names_compile};transitional_error_aliases_rejected={transitional_error_aliases_rejected};retired_roots_rejected={retired_roots_rejected};raw_devices_rejected={raw_devices_rejected}"
    );
}
