#[cfg(test)]
mod address_adverse;
mod nonzero_base;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> TestResult {
    nonzero_base::run_case(hardware_interface_runtime::RegisterBusBase::new(
        0x1_0000_0000,
    ))
    .await?;
    println!("direct_register_copy=true;nonzero_base=true");
    Ok(())
}
