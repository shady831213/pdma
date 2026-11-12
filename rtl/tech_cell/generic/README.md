# Generic RAM providers

These technology-independent RTL providers implement the contracts in
`common_cell`. They are copied with their tests from the same upstream snapshot
recorded in `../../common_cell/README.md`. No physical macro, library, tool
deployment or process configuration is included.

Keeping providers separate lets consumers select a later technology implementation
through the existing generic module interface. `common_cell` does not depend on
this project; consumers and the common-cell tests select these providers explicitly.
Generic synthesis does not imply SRAM-macro PPA.

Use the integrating project's qualified compiler with generic module parameter
support. From this package directory, run:

```sh
VERYL_DUT_REUSE=0 veryl test
VERYL_DUT_REUSE=0 veryl test --backend cranelift
```
