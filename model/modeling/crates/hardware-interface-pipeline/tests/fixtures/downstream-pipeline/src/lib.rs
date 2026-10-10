//! A downstream consumer cannot connect a numeric output to a string input.
//!
//! ```compile_fail,E0271
//! use std::convert::Infallible;
//! use hip::{PipelineExt, Stage, StepOutcome};
//!
//! struct Source;
//! impl Stage<()> for Source {
//!     type Input = ();
//!     type Output = u32;
//!     type Error = Infallible;
//!     type Wait = ();
//!     type State = ();
//!     fn start((): ()) {}
//!     fn step(&mut self, _: &mut (), _: ()) -> StepOutcome<(), u32, Infallible, ()> {
//!         StepOutcome::Complete(Ok(42))
//!     }
//! }
//!
//! struct Sink;
//! impl Stage<()> for Sink {
//!     type Input = String;
//!     type Output = ();
//!     type Error = Infallible;
//!     type Wait = ();
//!     type State = String;
//!     fn start(input: String) -> String { input }
//!     fn step(&mut self, _: &mut (), _: String) -> StepOutcome<String, (), Infallible, ()> {
//!         StepOutcome::Complete(Ok(()))
//!     }
//! }
//!
//! let _hardware = Source.then(Sink);
//! ```
