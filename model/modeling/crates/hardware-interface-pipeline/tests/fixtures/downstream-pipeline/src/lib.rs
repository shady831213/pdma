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
//!     type State = ();
//!     fn start((): ()) {}
//!     fn step(&mut self, _: &mut (), _: &mut ()) -> StepResult<u32, Infallible> {
//!         Ok(StepOutcome::Complete(Ok(42)))
//!     }
//! }
//!
//! struct Sink;
//! impl Stage<()> for Sink {
//!     type Input = String;
//!     type Output = ();
//!     type Error = Infallible;
//!     type State = String;
//!     fn start(input: String) -> String { input }
//!     fn step(&mut self, _: &mut (), _: &mut String) -> StepResult<(), Infallible> {
//!         Ok(StepOutcome::Complete(Ok(())))
//!     }
//! }
//!
//! let _hardware = Source.then(Sink);
//! ```
