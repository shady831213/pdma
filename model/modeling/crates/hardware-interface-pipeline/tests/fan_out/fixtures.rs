use hardware_interface_pipeline::{FanPlan, FanPolicy, RunError, Stage, StepOutcome, StepResult};
#[derive(Default)]
pub(super) struct Context {
    pub sources: usize,
    pub attempts: Vec<usize>,
    pub deliveries: Vec<(usize, String)>,
    pub blocked: Vec<usize>,
    pub reset: bool,
}
#[derive(Debug)]
pub(super) struct Parcel(pub String);
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Receipt(pub usize);
pub(super) struct Request(pub Vec<(usize, Parcel)>);
pub(super) struct Source;
impl Stage<Context> for Source {
    type Input = Request;
    type Output = Request;
    type Error = &'static str;
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Option<Self::Input>,
    ) -> StepResult<Request, Self::Error> {
        let request = state.take().ok_or(RunError::Exhausted)?;
        context.sources += 1;
        Ok(StepOutcome::Complete(Ok(request)))
    }
}
pub(super) enum Behavior {
    Normal,
    Progress,
    Error,
    Exhausted,
}
pub(super) struct Destination {
    pub id: usize,
    pub behavior: Behavior,
    pub pending: (Option<Parcel>, bool),
}
impl Destination {
    pub(super) const fn new(id: usize) -> Self {
        Self {
            id,
            behavior: Behavior::Normal,
            pending: (None, false),
        }
    }
}
impl Stage<Context> for Destination {
    type Input = Parcel;
    type Output = Receipt;
    type Error = &'static str;
    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Receipt, Self::Error> {
        if self.pending.0.is_none() {
            self.pending = (input.take(), false);
        }
        if self.pending.0.is_none() {
            return Ok(StepOutcome::InProgress);
        }
        let state = &mut self.pending;
        context.attempts.push(self.id);
        if context.reset {
            state.0 = None;
            return Ok(StepOutcome::Complete(Err("reset")));
        }
        if context.blocked.contains(&self.id) {
            return Ok(StepOutcome::InProgress);
        }
        match self.behavior {
            Behavior::Error => {
                state.0 = None;
                return Ok(StepOutcome::Complete(Err("destination failed")));
            }
            Behavior::Exhausted => {
                self.behavior = Behavior::Normal;
                return Err(RunError::Exhausted);
            }
            Behavior::Progress if !state.1 => {
                state.1 = true;
                if let Some(parcel) = &mut state.0 {
                    parcel.0.push('!');
                }
                return Ok(StepOutcome::InProgress);
            }
            Behavior::Progress | Behavior::Normal => {}
        }
        let parcel = state.0.take().ok_or(RunError::Exhausted)?;
        context.deliveries.push((self.id, parcel.0));
        Ok(StepOutcome::Complete(Ok(Receipt(self.id))))
    }
}
pub(super) struct Routing;
impl FanPolicy<Request, Parcel, &'static str> for Routing {
    fn plan(&mut self, values: &[Option<Request>], _: &[bool]) -> Result<FanPlan, &'static str> {
        Ok(match values.first() {
            Some(Some(request)) => FanPlan {
                inputs: vec![0],
                outputs: request.0.iter().map(|(i, _)| *i).collect(),
            },
            Some(None) | None => FanPlan::default(),
        })
    }
    fn route(&mut self, mut values: Vec<Request>) -> Result<Vec<Parcel>, &'static str> {
        Ok(values
            .pop()
            .ok_or("missing request")?
            .0
            .into_iter()
            .map(|(_, parcel)| parcel)
            .collect())
    }
}
pub(super) fn request(indices: &[usize]) -> Request {
    Request(
        indices
            .iter()
            .map(|&i| (i, Parcel(format!("payload {i}"))))
            .collect(),
    )
}
pub(super) fn finish<S: Stage<Context>>(
    hardware: &mut S,
    state: &mut hardware_interface_pipeline::Operation<S, Context>,
    context: &mut Context,
) -> StepResult<S::Output, S::Error> {
    for _ in 0..100 {
        match state.step(hardware, context) {
            Ok(StepOutcome::InProgress) => {}
            terminal => return terminal,
        }
    }
    Err(RunError::BudgetExhausted)
}
