# Hardware interface IRQ

Independent interrupt capabilities for hardware adapters and software consumers.
`InterruptEnvelope` identifies a notification's vector, `InterruptSource` separates
non-consuming readiness from serial claim, and `InterruptHandler` borrows persistent
handler state. These contracts require `std` and Send futures; they impose no CPU
identity, executor, scheduling policy, registry, or hardware handler storage.
Vector ordering is not required; consumers choose their own dispatch strategy.

Disable default features to use the traits without a normal Tokio dependency.
The `plain_irq` test demonstrates a timer source and counter handler driven by
standard-library polling, without a software CPU or dispatcher.

The default `tokio` feature adds `pending_interrupt`, `InterruptEmitter`, and
`PendingInterrupt`, using only Tokio's synchronization feature. This creates one
fixed coalescing destination, not a payload queue. Only the emitter raises; only
the source waits and claims. Waiting is non-consuming and cancellation-safe.
Repeated raises coalesce until claim; a raise after claim remains pending. Cloned
sources compete for one notification. The source keeps the channel alive after
all emitters drop; the consumer controls shutdown. Device causes, masks, routing,
and hardware-step wakeup policy remain outside this primitive.

These package names are provisional and publication is disabled.
