#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]

mod compose;
mod outcome;
mod stage;

pub use compose::{PipelineExt, Then};
pub use outcome::StepOutcome;
pub use stage::Stage;
