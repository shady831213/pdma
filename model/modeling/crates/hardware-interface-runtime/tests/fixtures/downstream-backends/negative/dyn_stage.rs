use hip::{PipelineExt, Stage, StepOutcome, StepResult};

struct Terminal;

impl Stage<()> for Terminal {
    type Input = ();
    type Output = ();
    type Error = ();
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, _: &mut (), _: &mut ()) -> StepResult<(), ()> {
        Ok(StepOutcome::Complete(Ok(())))
    }
}

fn main() {
    let stage: Box<dyn Stage<(), Input = (), Output = (), Error = (), State = ()>> =
        Box::new(Terminal);
    let _composed = stage.then(Terminal);
}
