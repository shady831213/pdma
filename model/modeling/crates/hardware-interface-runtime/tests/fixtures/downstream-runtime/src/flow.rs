mod adverse;
mod happy;

use hir::TraceBuffer;
use std::error::Error;

type FlowResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Final consumer observables derived from one public capability flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// Exact happy-path terminal line.
    pub happy_line: String,
    /// Exact adverse-path terminal line.
    pub adverse_line: String,
}

struct HappyFlow {
    memory: hir::SharedMemory,
    region: hir::MemoryRegion,
    alignment: hir::MemoryAlignment,
    trace: TraceBuffer<(usize, &'static str)>,
    deliveries: usize,
}

/// Runs the full standalone capability flow using only the renamed runtime facade.
pub async fn run_flow() -> Result<RunReport, Box<dyn Error + Send + Sync>> {
    let happy = happy::run().await?;
    let adverse = adverse::run(&happy.memory, happy.region, happy.alignment).await?;
    let order = checked_order(&happy.trace)?;
    Ok(RunReport {
        happy_line: format!(
            "memory=64:first=0x11:last=0xee;register=write@0x20:[de,ad,be,ef],read@0x10:[78,56,34,12];order={order};deliveries={};trace-sequences=0..7",
            happy.deliveries
        ),
        adverse_line: adverse,
    })
}

fn checked_order(trace: &TraceBuffer<(usize, &'static str)>) -> FlowResult<String> {
    let events = trace.events();
    require(events.len() == 8, "trace did not retain eight events")?;
    let order = events
        .iter()
        .map(|(_, label)| *label)
        .collect::<Vec<_>>()
        .join(",");
    require(
        order
            == "memory-write,memory-read,register-write,register-read,main-before,handler-start,handler-finish,main-after",
        "trace order changed",
    )?;
    require(
        events.iter().map(|(sequence, _)| *sequence).eq(0..8),
        "trace sequences changed",
    )?;
    Ok(order)
}

fn require(condition: bool, message: &'static str) -> FlowResult<()> {
    if condition {
        Ok(())
    } else {
        Err(Box::new(FlowError(message)))
    }
}

fn byte_hash(bytes: &[u8]) -> u64 {
    bytes.iter().map(|byte| u64::from(*byte)).sum()
}

#[derive(Debug)]
struct FlowError(&'static str);

impl std::fmt::Display for FlowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for FlowError {}
