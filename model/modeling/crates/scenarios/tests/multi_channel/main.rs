//! Multi-channel acceptance through one complete CPU-local software program.

mod cases;
pub mod hardware;
pub mod harness;
pub mod software;

use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy, Debug)]
enum Scenario {
    CopyInFlight,
    QueuedChannel,
    SuccessWithoutEvents,
    WholeDevice,
}
