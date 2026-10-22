//! Case-owned expectations shared by linear and scatter/gather transfers.

mod faults;
mod linear;
mod scatter_gather;

use crate::{
    harness::{self, FaultInjection, TestResult},
    software,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{ChannelFatalCode, Segment};

async fn copy(response_fault: bool, input: software::Input<'_>) -> TestResult<Vec<Segment>> {
    let injection = if response_fault {
        FaultInjection::EventWrite
    } else {
        FaultInjection::None
    };
    let report = harness::run(injection, input).await?;
    let software::Outcome {
        event,
        fatal,
        completed,
        destination,
    } = report.program?;
    assert!(report.worker.is_cancelled());
    let mut copied = Vec::with_capacity(input.data.len());
    for segment in &destination {
        if segment.length == 0 {
            continue;
        }
        let address = MemoryAddress::new(segment.address);
        let region = report
            .storage
            .regions()?
            .into_iter()
            .find(|region| region.base() == address)
            .ok_or_else(|| std::io::Error::other("destination allocation is missing"))?;
        assert_eq!(region.alignment(), MemoryAlignment::ONE);
        let mut bytes = vec![0; usize::try_from(segment.length)?];
        report
            .storage
            .read(address, MemoryAlignment::ONE, &mut bytes)?;
        copied.extend(bytes);
    }
    assert_eq!(copied, input.data);
    assert_eq!(report.summary.deliveries(), 1);
    assert!(!report.irq);
    assert!(completed);
    assert_eq!(event, None);
    assert_eq!(fatal, ChannelFatalCode::None);
    println!("copy: copied={copied:?}, fence observed, no error event");
    Ok(destination)
}
