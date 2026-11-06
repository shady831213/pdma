# jarvisuk allocator dependency patch

PDMA uses [jarvisuk](https://github.com/shady831213/jarvisuk) as its UVM memory
allocator. Its Apache-2.0 license applies to the upstream source and the derived
allocator patch. The preparation helper copies the dependency with its `LICENSE`
and other notices, excludes `.git`, and leaves the supplied checkout unchanged.

`patches/jarvisuk-allocator-free.patch` fixes only `jvs_memory_allocator::free`.
The original coalescing path can use a still-allocated lower neighbor's start
address, or leave the newly created free block at its default address when there
is no lower neighbor. The patch initializes the free block to the released
interval and merges adjacent entries only from the free queue. Allocation,
alignment, backing storage and the public allocator API remain unchanged.

The accepted upstream `src/memory/jvs_memory_allocator.sv` SHA-256 is
`8e893c07f27538a586687b00692db982004ecfa775280b47d73b57d936d0f9ee`.
The patched file SHA-256 is
`e85bb69f54ca1f756996aee29d530a6b82bad53d4cdefcd91e05f0117372668b`.
A different upstream allocator requires explicit patch review; the helper rejects
it instead of attempting a fuzzy application.

```sh
bash verification/dma/flow/prepare-jarvisuk.sh "$JVSUK_HOME" "$BUILD/jarvisuk"
export JVSUK_HOME="$BUILD/jarvisuk"
```

The destination must not exist. It receives the copied dependency plus
`pdma-allocator-before.sha256`, `pdma-allocator-after.sha256`,
`pdma-allocator-patch.sha256` and `pdma-allocator-patch.log`. Patch application uses
`--fuzz=0`; the compile flow should hash the copied dependency as a build input.

Regression scenarios use the real `jvs_pkg` with heap `[0x1000, 0x10ff]`:

- Reserve adjacent 32-byte blocks at `0x1000` and `0x1020`, free the second,
  reject reallocation at the live first block, and reuse the second interval.
- Free the first with the second still live, reject the live and outside-heap
  addresses, then reuse the first interval.
- Free the sole 32-byte heap-start allocation with a free upper neighbor and
  reject allocation at address zero.
- Fill the heap with two 128-byte blocks and free each without overlapping the
  remaining live block.

Each scenario ends by allocating the complete heap, rejecting another allocation
within it, freeing it and allocating the complete heap again. The first three
scenarios fail before the patch; all four must pass after it. Simulator exit code
alone is insufficient: require each scenario's explicit pass marker and reject
fatal diagnostics.
