//! A downstream consumer cannot connect a numeric output to a string input.
//!
//! ```compile_fail,E0271
//! use std::convert::Infallible;
//! use hip::{StepResult, PipelineExt, Stage, StepOutcome};
//!
//! struct Source;
//! impl Stage<()> for Source {
//!     type Input = ();
//!     type Output = u32;
//!     type Error = Infallible;
//!     fn step(&mut self, _: &mut (), input: &mut Option<()>) -> StepResult<u32, Infallible> {
//!         let Some(()) = input.take() else { return Ok(StepOutcome::InProgress); };
//!         Ok(StepOutcome::Complete(Ok(42)))
//!     }
//! }
//!
//! struct Sink;
//! impl Stage<()> for Sink {
//!     type Input = String;
//!     type Output = ();
//!     type Error = Infallible;
//!     fn step(&mut self, _: &mut (), input: &mut Option<String>) -> StepResult<(), Infallible> {
//!         let Some(_) = input.take() else { return Ok(StepOutcome::InProgress); };
//!         Ok(StepOutcome::Complete(Ok(())))
//!     }
//! }
//!
//! let _hardware = Source.then(Sink);
//! ```
