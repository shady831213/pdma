#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]

mod after_each;
mod backend;
mod compose;
mod executor;
mod operation;
mod outcome;
mod stage;

pub use after_each::AfterEach;
pub use backend::AsyncInteractionBackend;
pub use compose::{PipelineExt, Then, ThenState};
pub use executor::{
    AsyncExecutor, AsyncRunError, ImmediateExecutor, RunError, RunOutcome, RunResult, StepExecutor,
};
pub use operation::Operation;
pub use outcome::StepOutcome;
pub use stage::Stage;
