//! Canonical host-core downstream surface tests.

include!("host_core_surface/compiler.rs");
#[path = "host_core_surface/support.rs"]
mod support;

use hardware_interface_runtime::{CpuContextError, SoftwareCpu};
use pdma_model_contract::semantic::{CommandId, CompletionRecord, CopyRequest};
use pdma_model_host::{
    DriverPort, ModelError,
    adapters::AdapterError,
    compat::CompatError,
    core::{AsyncDriver, DriverError, ManualDriver, PortError},
    interrupt::InterruptError,
    register::RegisterError,
};
use std::{collections::VecDeque, error::Error, future::poll_fn, task::Poll};

#[derive(Default)]
struct ProbePort {
    allocations: usize,
    queues: usize,
    submissions: usize,
    completions: usize,
    irq_calls: usize,
    pending: VecDeque<CompletionRecord>,
    submit_blocked: bool,
    doorbell_blocked: bool,
}

const fn accepts_driver_port<P: DriverPort + ?Sized>(_port: &P) {}

#[test]
fn public_host_layers_expose_cause_preserving_errors() {
    // Given: every host layer receives one legacy compatibility failure.
    let driver = DriverError::from(ModelError::SoftwareCpuInactive);
    let register = RegisterError::Access(PortError::SubmissionState);
    let interrupt = InterruptError::from(ModelError::DuplicateIrqVector(
        pdma_model_contract::semantic::IrqVector::new(7),
    ));
    let adapter = AdapterError::from(ModelError::DmaAllocationExhausted);
    let compat = CompatError::from(ModelError::RingFull);

    // When: downstream code inspects each public error boundary.
    let source_count = [&driver as &dyn Error, &register, &adapter, &compat]
        .into_iter()
        .filter(|error| error.source().is_some())
        .count();

    // Then: explicit variants retain their specified cause behavior.
    assert_eq!(driver, DriverError::InactiveCpu(CpuContextError::Inactive));
    assert_eq!(
        interrupt,
        InterruptError::DuplicateVector(pdma_model_contract::semantic::IrqVector::new(7))
    );
    assert_eq!(source_count, 4);
    assert!(interrupt.source().is_none());
    accepts_driver_port(&ProbePort::default());
}

#[tokio::test(flavor = "current_thread")]
async fn generic_port_drives_both_non_generic_facades() {
    // Given: a contract-only port and one driver of each ownership mode.
    let mut port = ProbePort::default();
    let cpu = SoftwareCpu::new();
    let mut async_driver = cpu
        .run_main(AsyncDriver::setup_on(&mut port, 1, 2))
        .await
        .expect("async setup");
    let mut clone = async_driver.clone();
    let manual_driver = ManualDriver::setup_on(&mut port, 1, 1)
        .await
        .expect("manual setup");
    let allocations_before_async = port.allocations;
    let async_buffer = async_driver
        .allocate_buffer_on(&mut port, 4)
        .await
        .expect("async allocation");
    let async_allocations = port.allocations - allocations_before_async;
    let allocations_before_manual = port.allocations;
    let manual_buffer = manual_driver
        .allocate_buffer_on(&mut port, 4)
        .await
        .expect("manual allocation");
    let manual_allocations = port.allocations - allocations_before_manual;

    // When: clones share depth-one credit, reclaim it, and submit again.
    let request = CopyRequest::new(async_buffer, manual_buffer, 4);
    let first = cpu
        .run_main(async_driver.submit_copy_on(&mut port, request))
        .await;
    let full = cpu.run_main(clone.submit_copy_on(&mut port, request)).await;
    let first_completion = cpu.run_main(clone.poll_completion_on(&mut port)).await;
    let second = cpu
        .run_main(async_driver.submit_copy_on(&mut port, request))
        .await;
    let second_completion = cpu
        .run_main(async_driver.poll_completion_on(&mut port))
        .await;

    // Then: the fake observes only contract values and both facades preserve accounting.
    assert_eq!(first, Ok(CommandId::new(0)));
    assert_eq!(full, Err(ModelError::DriverQueueFull));
    assert!(matches!(first_completion, Ok(Some(_))));
    assert_eq!(second, Ok(CommandId::new(1)));
    assert!(matches!(second_completion, Ok(Some(_))));
    let submissions = port.submissions;
    let completions = port.completions;
    let shared_accounting = full == Err(ModelError::DriverQueueFull);
    let reserved_commands = submissions.saturating_sub(completions);
    let reserved_credits = reserved_commands;
    let compile_surface =
        verify_downstream_compile_surface().expect("downstream compiler proof executes");
    let manual_irq_methods = compile_surface.manual_irq_methods;
    let device_types = compile_surface.device_types;
    assert_eq!((port.allocations, submissions, completions), (2, 2, 2));
    let cancellation = cancellation_and_cpu_observables_are_live()
        .await
        .expect("cancellation fixtures set up");
    let inactive_cpu = matches!(cancellation.inactive, Err(ModelError::SoftwareCpuInactive));
    let wrong_cpu = matches!(cancellation.wrong, Err(ModelError::SoftwareCpuMismatch));
    let preaccept_dropped = cancellation.commits_before_accept == 0;
    let postaccept_dropped = cancellation.accepted_commands == 1;
    let credits_before_accept = usize::from(cancellation.commits_before_accept > 0);
    let accepted_credits = usize::from(cancellation.retained == Err(ModelError::DriverQueueFull));
    let recovered = cancellation.recovered == Ok(CommandId::new(1));
    assert_eq!((async_allocations, manual_allocations), (1, 1));
    assert!(shared_accounting);
    assert_eq!((reserved_commands, reserved_credits), (0, 0));
    assert_eq!((manual_irq_methods, device_types), (0, 0));
    assert!(inactive_cpu);
    assert!(wrong_cpu);
    assert!(preaccept_dropped);
    assert!(postaccept_dropped);
    assert!(recovered);
    assert_eq!(
        (cancellation.commits_before_accept, credits_before_accept),
        (0, 0)
    );
    assert_eq!((cancellation.accepted_commands, accepted_credits), (1, 1));
    println!(
        "async_allocations={async_allocations};manual_allocations={manual_allocations};submissions={submissions};completions={completions};shared_accounting={shared_accounting};reserved_commands={reserved_commands};reserved_credits={reserved_credits};manual_irq_methods={manual_irq_methods};device_types={device_types}"
    );
    println!(
        "inactive_cpu={};wrong_cpu={};preaccept_dropped={};postaccept_dropped={};commits_before_accept={};credits_before_accept={};accepted_commands={};accepted_credits={};recovered={}",
        inactive_cpu,
        wrong_cpu,
        preaccept_dropped,
        postaccept_dropped,
        cancellation.commits_before_accept,
        credits_before_accept,
        cancellation.accepted_commands,
        accepted_credits,
        recovered
    );
}

struct CancellationObservables {
    inactive: Result<AsyncDriver, ModelError>,
    wrong: Result<Option<pdma_model_contract::semantic::CompletionRecord>, ModelError>,
    retained: Result<CommandId, ModelError>,
    recovered: Result<CommandId, ModelError>,
    commits_before_accept: usize,
    accepted_commands: usize,
}

async fn cancellation_and_cpu_observables_are_live() -> Result<CancellationObservables, ModelError>
{
    // Given: independent depth-one drivers with pre-accept and post-accept gates.
    let cpu = SoftwareCpu::new();
    let mut pre_port = ProbePort::default();
    let mut pre_driver = cpu
        .run_main(AsyncDriver::setup_on(&mut pre_port, 1, 1))
        .await?;
    let request = CopyRequest::new(
        pdma_model_contract::semantic::BufferId::new(0),
        pdma_model_contract::semantic::BufferId::new(1),
        1,
    );
    pre_port.block_submit();
    let mut pending_submit = Box::pin(pre_driver.submit_copy_on(&mut pre_port, request));
    cpu.run_main(poll_fn(|context| {
        assert!(pending_submit.as_mut().poll(context).is_pending());
        Poll::Ready(())
    }))
    .await;
    drop(pending_submit);
    let commits_before_accept = pre_port.submissions;
    pre_port.allow_submit();
    let recovered = cpu
        .run_main(pre_driver.submit_copy_on(&mut pre_port, request))
        .await;

    // When: a second driver is cancelled after acceptance but before doorbell completion.
    let mut post_port = ProbePort::default();
    let mut post_driver = cpu
        .run_main(AsyncDriver::setup_on(&mut post_port, 1, 1))
        .await?;
    post_port.block_doorbell();
    let mut pending_doorbell = Box::pin(post_driver.submit_copy_on(&mut post_port, request));
    cpu.run_main(poll_fn(|context| {
        assert!(pending_doorbell.as_mut().poll(context).is_pending());
        Poll::Ready(())
    }))
    .await;
    drop(pending_doorbell);
    let accepted_commands = post_port.submissions;
    let retained = cpu
        .run_main(post_driver.submit_copy_on(&mut post_port, request))
        .await;
    let inactive = AsyncDriver::setup_on(&mut ProbePort::default(), 1, 1).await;
    let wrong_cpu = SoftwareCpu::new()
        .run_main(post_driver.poll_completion_on(&mut post_port))
        .await;

    // Then: both cancellation boundaries and CPU precedence are directly observed.
    assert_eq!(commits_before_accept, 0);
    assert_eq!(recovered, Ok(CommandId::new(1)));
    assert_eq!(accepted_commands, 1);
    assert_eq!(retained, Err(ModelError::DriverQueueFull));
    assert!(matches!(inactive, Err(ModelError::SoftwareCpuInactive)));
    assert!(matches!(wrong_cpu, Err(ModelError::SoftwareCpuMismatch)));
    Ok(CancellationObservables {
        inactive,
        wrong: wrong_cpu,
        retained,
        recovered,
        commits_before_accept,
        accepted_commands,
    })
}
