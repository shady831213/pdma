use super::fixtures::{Parcel, Request};
use hw_model_pipeline::{FanPlan, FanPolicy};
pub(super) enum Reject {
    Plan,
    Route,
}
impl FanPolicy<Request, Parcel, &'static str> for Reject {
    fn plan(&mut self, _: &[Option<Request>], _: &[bool]) -> Result<FanPlan, &'static str> {
        match self {
            Self::Plan => Err("plan failed"),
            Self::Route => Ok(FanPlan {
                inputs: vec![0],
                outputs: vec![0],
            }),
        }
    }
    fn route(&mut self, _: Vec<Request>) -> Result<Vec<Parcel>, &'static str> {
        Err("route failed")
    }
}
