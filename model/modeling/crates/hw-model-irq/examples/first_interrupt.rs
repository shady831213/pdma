//! Trigger, wait, and claim one coalescing interrupt.
//!
//! Run: `cargo run -p hw-model-irq --example first_interrupt`.
//! Requires the default `tokio` feature. This demonstrates the interrupt source;
//! handler registration and software execution belong to the consuming runtime.
use hw_model_irq::{InterruptEnvelope, InterruptSource, interrupt_wire};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Notice(u8);

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> u8 {
        self.0
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Assembly gives the emitter to hardware and the receiver to software dispatch.
    // Notice identifies this wire; raise() does not enqueue a new payload each time.
    let (emitter, mut receiver) = interrupt_wire(Notice(3));
    emitter.raise();
    emitter.raise(); // Repeated triggers coalesce until a claim.
    receiver.wait_ready().await?; // Readiness does not consume the pending latch.
    assert_eq!(receiver.claim(()).await?, Some(Notice(3)));
    assert_eq!(receiver.claim(()).await?, None);
    println!("two raises coalesced into one claimed interrupt");
    Ok(())
}
