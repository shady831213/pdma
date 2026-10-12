#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]
mod backend;
mod combinators;
mod executor;
mod operation;
mod outcome;
mod stage;
pub use backend::AsyncInteractionBackend;
pub use combinators::{
    AfterEach, AfterEachState, Arbitrate, Arbitration, BranchError, Dispatch, Fan, FanIn, FanOut,
    FanPlan, FanPolicy, FanState, MapError, PipelineExt, Repeat, Route, Then, ThenState,
};
pub use executor::{AsyncExecutor, AsyncRunError, ImmediateExecutor, RunError, StepExecutor};
pub use operation::Operation;
pub use outcome::{StepOutcome, StepResult};
pub use stage::Stage;
