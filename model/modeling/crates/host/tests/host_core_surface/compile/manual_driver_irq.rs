use pdma_model_contract::IrqNotification;
use pdma_model_host::{DriverPort, ManualDriver};

async fn manual_driver_has_no_irq_service<P: DriverPort>(
    driver: &mut ManualDriver,
    port: &mut P,
    notification: IrqNotification,
) {
    let _ = driver.service_irq_on(port, notification).await;
}

fn main() {}
