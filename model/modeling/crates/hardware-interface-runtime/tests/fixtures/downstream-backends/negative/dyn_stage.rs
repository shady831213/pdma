use hip::{PipelineExt, Stage, StepOutcome};

struct Terminal;

impl Stage<()> for Terminal {
    type Output = ();
    type Error = ();
    type Wait = ();

    fn step(&mut self, _: &mut ()) -> StepOutcome<(), (), ()> {
        StepOutcome::Complete(Ok(()))
    }
}

fn main() {
    let stage: Box<dyn Stage<(), Output = (), Error = (), Wait = ()>> = Box::new(Terminal);
    let _composed = stage.then(|()| Terminal);
}
