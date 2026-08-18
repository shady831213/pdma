use pdma_model_contract::semantic::IrqNotification;
use pdma_model_host::{DriverPort, ManualDriver};

async fn reject<P: DriverPort>(
    driver: &mut ManualDriver,
    port: &mut P,
    notification: IrqNotification,
) {
    let _ = driver.service_irq_on(port, notification).await;
}

fn main() {}
