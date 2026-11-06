# Generic RAM providers

These technology-independent RTL providers implement the contracts in
`common_cell`. They are copied with their tests from the same upstream snapshot
recorded in `../../common_cell/README.md`. No physical macro, library, tool
deployment or process configuration is included.

Keeping providers separate lets consumers select a later technology implementation
through the existing generic module interface. `common_cell` does not depend on
this project; its tests select these providers explicitly. The DMA does not yet
instantiate these RAMs, and generic synthesis does not imply SRAM-macro PPA.

From this directory, run:

```sh
VERYL_DUT_REUSE=0 bash ../../../tools/rtl-flow/veryl.sh test
VERYL_DUT_REUSE=0 bash ../../../tools/rtl-flow/veryl.sh test --backend cranelift
```
