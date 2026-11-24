# jarvisuk dependency

PDMA uses [jarvisuk](https://github.com/shady831213/jarvisuk) as its UVM memory,
clock/reset and interrupt library. Its Apache-2.0 license remains applicable.

The allocator free/coalescing correction and its regression are upstream on
[master at bd24e3891d502d9fca8b85993260174f032bca3a](https://github.com/shady831213/jarvisuk/commit/bd24e3891d502d9fca8b85993260174f032bca3a).
The dependency lock pins the selected JarvisUK revision and the allocator's
SHA-256. The DMA bench uses instance-sized pin interfaces from that revision.

```sh
revision=$(python3 -c 'import json; print(json.load(open("verification/dma/dependencies/jarvisuk.lock.json"))["revision"])')
GIT_MASTER=1 git -C "$JVSUK_HOME" fetch origin "$revision"
bash verification/dma/flow/prepare-jarvisuk.sh "$JVSUK_HOME" "$BUILD/jarvisuk"
export JVSUK_HOME="$BUILD/jarvisuk"
```

Preparation archives the pinned commit from the supplied Git checkout into a new
destination, retaining licenses and notices and excluding Git metadata. It does
not alter the checkout or incorporate its uncommitted changes. Missing pinned
commits, mismatched allocator content and existing destinations are rejected.
The build receives revision/pin receipts; compilation fingerprints the actual
dependency inputs. Dependency upgrades require an explicit lock update.

The upstream `jvs_memory_allocator_free` regression tests four cases using the
real public allocator and memory model: live lower-neighbour isolation, a
heap-start release with a live upper neighbour, a heap-start release with a free
upper neighbour, and full-heap reclamation while other blocks remain live.
Every case checks outside-heap rejection and two complete heap reuse cycles.
All four fail before the correction and pass after it under UVM 1.1 and 1.2.
Require each `JVS_ALLOCATOR_FREE_PASS case=N` marker and zero UVM errors/fatals;
simulator exit status alone is not sufficient.
