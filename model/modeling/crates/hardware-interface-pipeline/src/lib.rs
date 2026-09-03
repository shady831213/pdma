#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]

mod compose;
mod executor;
mod outcome;
mod stage;

pub use compose::{PipelineExt, Then};
pub use executor::{ImmediateExecutor, RunError, RunOutcome, RunResult, StepExecutor};
pub use outcome::StepOutcome;
pub use stage::Stage;
