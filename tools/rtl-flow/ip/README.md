# Veryl package resources

Veryl owns dependency resolution. The snapshot flow requests native
`metadata --format json --format-version 2`, using resolved provider names, paths
and lock identities. It does not parse the lockfile or maintain another graph.
Native metadata v1 supplies each project's declared source directories for input
fingerprinting; it does not change the compiler's source selection.

An IP package may opt in through its own `Veryl.toml`:

```toml
[metadata.rtl_flow]
entry = "constraints/ip.tcl"
files = ["verification/cdc_custody.sv"]
```

The entry's parent is a dedicated resource directory; all its ordinary files are
copied. Optional `files` select additional package-relative resources. Missing
files, escaping paths, resource-directory symlinks and duplicate resolved provider
identities fail. Entries at the project root are rejected to avoid copying the
whole checkout. No absolute source/cache path is embedded in the resource manifest.

Snapshots carry `ip/`, `ip-manifest.tcl` and `ip-manifest.json` with checksums. A
derived simulation or qualification snapshot inherits those resources with
`bundle.py copy`. Reusing an output directory or replacing existing resources
fails. Resource consumers use the snapshot copy even if the original checkout
has disappeared. A consumer can locate a declared passive checker:

```sh
python3 tools/rtl-flow/ip/bundle.py locate "$snapshot" common_cell verification/cdc_custody.sv
```

## Constraint provider contract

The Tcl entry defines functions and returns an export dictionary. Loading it must
not create clocks, apply timing exceptions or waive findings. Each resolved
provider loads in its own Tcl namespace, so multiple versions of the same project
can coexist. Only an explicit integration binding invokes a provider.

| Export | Arguments | Result |
| --- | --- | --- |
| `validate` | binding dictionary | Validate component-specific parameters without accessing a tool database |
| `describe` | instance path, source clock, destination clock, binding dictionary | `paths` and `chains` belonging to this exact instance |
| `synchronizer_cells` | list of chains | Actual synchronizer register collection, checking expected structure |
| `set_source_names` (optional) | resolver command | Supply native RTL-origin names for current physical objects after optimization |

A path supplies `scope`, `kind`, `source`, `destination`, `from`, `to` and
`max_delay`. An optional `payload` collection identifies the actual held-data
registers for a separate timing audit. A chain is `{scope clock expected_count}`.
The provider owns private hierarchy, payload selection and synchronizer shape;
the generic adapter neither guesses names nor special-cases FIFO/handshake types.
Descriptions resolve native collections again after mapping, never retaining stale
objects across netlist transformations.
The optional name resolver returns a list of source names for one current object.
The tool adapter supplies it with `rtl_ip::configure_names`; the package keeps
ordinary object-name lookup when none is supplied. It does not retain hierarchy,
create an exception or grant review approval. Genus uses its native `original_name`
attribute instead of guessing flattened names; other adapters keep their own
object query mechanisms.

The integration profile supplies clocks, reset groups, interface ownership and
explicit `crossings`, for example:

```tcl
crossings {
    soc/queue0 {
        provider common_cell cell AsyncQueue depth 8
        source producer_clk destination consumer_clk max_delay 0.5
    }
}
```

The budget uses the integration's SDC units. Generic validation checks clock and
budget relationships; provider validation checks its own cell/parameter contract.
Public standalone cell procedures remain available by sourcing the package entry
directly when an integration already owns resource loading.

## Review ownership

Package ownership is not review approval. The current common-cell package exports
no waiver. Full-design dispositions belong to the integrating project, selected
explicitly and bound to RTL, configuration, resources, tool and review identities.
Raw findings are preserved; missing, extra or mismatched findings fail. Future
component-local review templates may travel as package files, but their assumptions
and instance bindings must be checked before use. Platform clocks, electrical
budgets, technology inputs and full-design review results do not belong to a
reusable cell package.

Run `python3 tools/rtl-flow/tests/ip_test.py` for transport, namespace, path,
relocation and immutable-output checks. These use the pinned Veryl metadata command
and Tcl, without commercial tools. Native implementation tests separately qualify
real object binding and effective timing budgets.
