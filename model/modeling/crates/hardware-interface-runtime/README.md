# Hardware interface runtime

Workspace-private, runtime-neutral mechanics shared by hardware-interface models.
It owns checked bulk memory storage, raw async register access, synchronous ordered
observation delivery, and software CPU execution contexts. Address allocation, register
maps and effects, device IDs, epochs, logical zero mappings, recording collectors, actors,
callbacks, dispatch policy, and timing remain consumer policy.

`SoftwareCpu` treats each real main `Poll::Pending` as a segment boundary while retaining
FIFO position during gate acquisition. `run_handler_turn` holds that gate for one complete
handler future. `CpuOwner` and `CpuLocal<T>` expose opaque validation and synchronous state
access without exposing CPU identity, locks, or await-spanning borrows.

Observation reservations are affine committed state. Consumers reserve only after every
fallible check, inside a non-awaiting commit section, then release effect locks and call
`ObservationReservation::emit` in the same non-cancellable tail. Dropping a reservation,
emitting a mismatched range, synchronization poison, or observer panic permanently faults
the gate and wakes all waiters; missing committed observations are never skipped.
