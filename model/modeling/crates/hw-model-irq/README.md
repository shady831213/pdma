# hw-model-irq

Independent interrupt capabilities for hardware adapters and software consumers.
`InterruptEnvelope` identifies a notification's vector, `InterruptSource` separates
non-consuming readiness from serial claim, and `InterruptHandler` borrows persistent
handler state. These contracts require `std` and Send futures; they impose no CPU
identity, executor, scheduling policy, registry, or hardware handler storage.
Vector ordering is not required; consumers choose their own dispatch strategy.

Disable default features to use the traits without a normal Tokio dependency.
The `plain_irq` test demonstrates a timer source and counter handler driven by
standard-library polling, without a software CPU or dispatcher.

The default `tokio` feature adds `interrupt_wire`, `InterruptEmitter`, and
`InterruptReceiver`, using only Tokio's synchronization feature. This creates one
fixed coalescing pending latch with claim, not an electrical assert/deassert level
or a payload queue. Only the emitter raises; only
the receiver waits and claims. Waiting is non-consuming and cancellation-safe.
Repeated raises coalesce until claim; a raise after claim remains pending. Cloned
receivers compete for one notification. The receiver keeps the channel alive after
all emitters drop; the consumer controls shutdown. Device causes, masks, routing,
and hardware-step wakeup policy remain outside this primitive.

These package names are provisional and publication is disabled.

## First interrupt

Run `cargo run -p hw-model-irq --example first_interrupt`.
With default features disabled, use the generic traits directly as shown in
`tests/plain_irq.rs`: `cargo test -p hw-model-irq --no-default-features`.

```rust
# #[cfg(feature = "tokio")]
# mod example {
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
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (emitter, mut receiver) = interrupt_wire(Notice(3));
    emitter.raise();
    emitter.raise(); // Repeated triggers coalesce until a claim.
    receiver.wait_ready().await?; // Readiness does not consume the pending latch.
    assert_eq!(receiver.claim(()).await?, Some(Notice(3)));
    assert_eq!(receiver.claim(()).await?, None);
    println!("two raises coalesced into one claimed interrupt");
    Ok(())
}
# }
# fn main() -> Result<(), Box<dyn std::error::Error>> {
#     #[cfg(feature = "tokio")]
#     example::main()?;
#     Ok(())
# }
```
