# Hardware interface register

Reusable numeric register access, checked address translation and ordered observations.
`RegisterAccess` exposes asynchronous endpoint-owned reads and writes without imposing
an executor. `ObservationOrder` tracks affine reservations; local and blocking backends
provide synchronous callback delivery with terminal fault retention.

The default `schema` feature adds `RegisterImage` and `RegisterBus`, backed by
`hardware-interface-schema` and closure-scoped `hardware-interface-storage` policies.
Disable default features for transports and observations without schema dependencies.
The normal dependency tree contains no Tokio or PDMA crates.

The blocking observation backend owns its synchronization. Closure reentry on the
same gate faults before acquiring its state mutex; independent gates may be nested.
Observer callbacks run outside the state lock and may inspect or reserve later work.
Recursive emission faults. Poisoning, abandoned reservations and callback panics are
terminal faults, including deferred abandonment inside a state closure; waiting
emitters wake to observe the fault.

Run the std-only example with:

```sh
cargo run -p hardware-interface-register --no-default-features --example register_observation
```
