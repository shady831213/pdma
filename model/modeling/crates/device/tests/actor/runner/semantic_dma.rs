mod async_surface;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) async fn exercise_semantic_dma_ranges_and_faults() -> TestResult {
    async_surface::exercise().await?;
    Ok(())
}
