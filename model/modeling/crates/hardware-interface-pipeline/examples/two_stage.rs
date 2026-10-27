//! Two externally stepped, typed stages with no device-model dependencies.
use std::convert::Infallible;

use hardware_interface_pipeline::{Stage, StepOutcome, StepResult, Then};

#[derive(Debug)]
struct Widen;

impl Stage<()> for Widen {
    type Input = u8;
    type Output = u16;
    type Error = Infallible;

    fn step(&mut self, (): &mut (), input: &mut Option<u8>) -> StepResult<u16, Infallible> {
        Ok(input.take().map_or(StepOutcome::InProgress, |value| {
            StepOutcome::Complete(Ok(u16::from(value)))
        }))
    }
}

#[derive(Debug)]
struct Encode;

impl Stage<()> for Encode {
    type Input = u16;
    type Output = [u8; 2];
    type Error = Infallible;

    fn step(&mut self, (): &mut (), input: &mut Option<u16>) -> StepResult<[u8; 2], Infallible> {
        Ok(input.take().map_or(StepOutcome::InProgress, |value| {
            StepOutcome::Complete(Ok(value.to_le_bytes()))
        }))
    }
}

fn main() {
    let mut pipeline = Then::new(Widen, Encode);
    let mut offer = Some(42);
    assert_eq!(
        pipeline.step(&mut (), &mut offer),
        Ok(StepOutcome::InProgress)
    );
    assert!(offer.is_none());
    assert_eq!(
        pipeline.step(&mut (), &mut offer),
        Ok(StepOutcome::Complete(Ok([42, 0])))
    );
}
