use hip::{PipelineExt, Stage, StepOutcome};

struct Terminal;

impl Stage<()> for Terminal {
    type Input = ();
    type Output = ();
    type Error = ();
    type Wait = ();
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, _: &mut (), _: ()) -> StepOutcome<(), (), (), ()> {
        StepOutcome::Complete(Ok(()))
    }
}

fn main() {
    let stage: Box<dyn Stage<(), Input = (), Output = (), Error = (), Wait = (), State = ()>> =
        Box::new(Terminal);
    let _composed = stage.then(Terminal);
}
