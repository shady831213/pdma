#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]

mod outcome;
mod stage;

pub use outcome::StepOutcome;
pub use stage::Stage;
