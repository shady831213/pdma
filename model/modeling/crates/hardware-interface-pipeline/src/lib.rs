#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]

mod backend;
mod compositor;
mod executor;
mod operation;
mod outcome;
mod stage;

pub use backend::AsyncInteractionBackend;
pub use compositor::{
    AfterEach, Combiner, CombinerState, PipelineExt, Repeat, Selection, Then, ThenState,
};
pub use executor::{
    AsyncExecutor, AsyncRunError, ImmediateExecutor, RunError, RunOutcome, RunResult, StepExecutor,
};
pub use operation::Operation;
pub use outcome::StepOutcome;
pub use stage::{ReadyStage, Stage};
