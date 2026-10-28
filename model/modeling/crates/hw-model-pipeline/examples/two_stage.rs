//! Two externally stepped, typed stages with no device-model dependencies.
//!
//! Run: `cargo run -p hw-model-pipeline --example two_stage`.
//! `Then` owns the edge between stages. Each stage consumes its input once;
//! the external caller supplies ticks rather than one stage driving another.
use std::convert::Infallible;

use hw_model_pipeline::{Stage, StepOutcome, StepResult, Then};

#[derive(Debug)]
struct Widen;

impl Stage<()> for Widen {
    type Input = u8;
    type Output = u16;
    type Error = Infallible;

    fn step(&mut self, (): &mut (), input: &mut Option<u8>) -> StepResult<u16, Infallible> {
        // Taking the offered value accepts it. An empty slot means no new work.
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
    // The unit context means these transforms need no shared hardware state.
    let mut pipeline = Then::new(Widen, Encode);
    let mut offer = Some(42);
    // Tick one runs Widen; Then retains its completed u16 for the next stage.
    assert_eq!(
        pipeline.step(&mut (), &mut offer),
        Ok(StepOutcome::InProgress)
    );
    assert!(offer.is_none());
    // Tick two runs Encode with that retained value, not a second input offer.
    assert_eq!(
        pipeline.step(&mut (), &mut offer),
        Ok(StepOutcome::Complete(Ok([42, 0])))
    );
}
