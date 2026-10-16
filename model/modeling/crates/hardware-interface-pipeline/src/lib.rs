#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]
mod backend;
mod combinators;
mod executor;
mod operation;
mod outcome;
mod stage;
pub use backend::AsyncInteractionBackend;
pub use combinators::{
    Arbitrate, Arbitration, BranchError, Dispatch, Fan, FanIn, FanOut, FanPlan, FanPolicy,
    MapError, PipelineExt, Route, Then,
};
pub use executor::{
    AsyncExecutor, AsyncRunError, HardwareDriver, ImmediateExecutor, RunError, StepExecutor,
};
pub use operation::Operation;
pub use outcome::{StepOutcome, StepResult};
pub use stage::Stage;
