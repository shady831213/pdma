use hip::{PipelineExt, Stage, StepOutcome, StepResult};

struct Terminal;

impl Stage<()> for Terminal {
    type Input = ();
    type Output = ();
    type Error = ();
    fn step(&mut self, _: &mut (), input: &mut Option<()>) -> StepResult<(), ()> {
        Ok(match input.take() {
            Some(()) => StepOutcome::Complete(Ok(())),
            None => StepOutcome::InProgress,
        })
    }
}

fn main() {
    let stage: Box<dyn Stage<(), Input = (), Output = (), Error = ()>> = Box::new(Terminal);
    let _composed = stage.then(Terminal);
}
