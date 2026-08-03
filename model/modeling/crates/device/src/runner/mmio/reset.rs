use super::RegisterBanks;
use crate::runner::state::RunnerState;

pub(in crate::runner) fn register_banks(state: &mut RunnerState) {
    state.mmio = RegisterBanks::new();
}
