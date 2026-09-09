# Historical PDMA scenarios

These sources are retained as behavior references for the PDMA rebuild, not as
current acceptance tests. They are excluded from the active Cargo workspace and
still depend on the deliberately removed contract, device, and host crates.
They cannot build standalone in this cleanup snapshot.

The complete runnable predecessor is on `backup/pdma-before-rebuild-20260909`
at `51707fe1d38e261a188e1d0ac14243c0caf58127`.
Reintroduce scenarios as the replacement model implements their behavior; do not
restore obsolete interfaces merely to compile these historical tests.
