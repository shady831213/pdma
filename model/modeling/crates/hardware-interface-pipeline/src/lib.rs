#![doc = "Runtime-neutral contracts for explicitly stepped hardware-interface pipelines."]

mod backend;
mod combinators;
mod executor;
mod operation;
mod outcome;
mod stage;

pub use backend::AsyncInteractionBackend;
pub use combinators::{
    AfterEach, Arbitrate, Arbitration, BranchError, Combine, DeliveryPlan, Dispatch, Distribute,
    FanIn, FanInState, FanOut, FanOutState, MapError, PipelineExt, Repeat, Route, Select, Then,
    ThenState,
};
pub use executor::{
    AsyncExecutor, AsyncRunError, ImmediateExecutor, RunError, RunOutcome, RunResult, StepExecutor,
};
pub use operation::Operation;
pub use outcome::StepOutcome;
pub use stage::{ReadyStage, Stage};
