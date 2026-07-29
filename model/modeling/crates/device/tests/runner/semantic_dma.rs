#[path = "semantic_dma/async_surface.rs"]
mod async_surface;
#[path = "semantic_dma/sync_surface.rs"]
mod sync_surface;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub async fn exercise_semantic_dma_ranges_and_faults() -> TestResult {
    async_surface::exercise().await?;
    sync_surface::exercise()?;
    Ok(())
}
