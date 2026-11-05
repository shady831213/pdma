/// One typed computation at any source, analysis or generation boundary.
///
/// Owned input supports transformations; borrowed input permits outputs that borrow
/// source metadata. Pass composition does not implicitly preserve prior results.
/// A pass can explicitly return prior data in a tuple, but then owns its validity
/// after any source change; only built-in stage types certify required invariants.
pub trait Pass<Input> {
    /// Input to the next selected pass.
    type Output;
    /// Failure owned by this pass.
    type Error;
    /// Executes this pass, consuming its input.
    ///
    /// # Errors
    /// Returns the pass's own error without invoking subsequent passes.
    fn run(self, input: Input) -> Result<Self::Output, Self::Error>;
}

/// Ordinary typed pass composition with no implicit stages or stored side data.
#[derive(Debug)]
pub struct Pipeline<Input> {
    input: Input,
}
impl<Input> Pipeline<Input> {
    /// Starts at any explicitly supplied processing state.
    pub const fn new(input: Input) -> Self {
        Self { input }
    }
    /// Transfers this state into one pass and retains only its output.
    ///
    /// # Errors
    /// Returns the pass's error without constructing the next pipeline state.
    pub fn run<P: Pass<Input>>(self, pass: P) -> Result<Pipeline<P::Output>, P::Error> {
        pass.run(self.input).map(Pipeline::new)
    }
    /// Borrows the current state without changing its validation guarantees.
    pub const fn input(&self) -> &Input {
        &self.input
    }
    /// Returns the current state for ordinary Rust composition or borrowing.
    pub fn into_inner(self) -> Input {
        self.input
    }
}
