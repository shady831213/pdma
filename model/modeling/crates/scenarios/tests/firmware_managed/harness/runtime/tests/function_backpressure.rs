use crate::harness::TestResult;
use pdma_model_contract::FunctionId;

#[tokio::test]
async fn pf_response_backpressure_preserves_vf_grant_until_release_can_respond() -> TestResult {
    super::backpressure::release_backpressure(FunctionId::new(1)).await
}
