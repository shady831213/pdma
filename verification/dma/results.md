# Scenario-level verification checkpoints

## Whole-DMA regression for register accesses during reset and byte strobes

The two new AXI VIP scenarios close all five previously uncovered lines in
`DmaRegisterBank`. The global-reset scenario holds an accepted payload write
response while software reads discovery, attempts to change IRQ enable and
checks the rejected write's readback. It checks that writing zero and repeating
the reset request both leave reset busy. After releasing the response, software
completes a fresh Copy/Fence and re-enables interrupts.

The byte-strobe scenario uses the generated register address and drives individual
AXI4-Lite beats. An empty write succeeds, each single-byte write returns SLVERR,
and a subsequent full register read confirms the original command base. A
misaligned read returns SLVERR and zero data. A valid write to either 32-bit
half changes exactly that half. RAL readback and a fresh Copy/Fence check the
software-visible result on both bus widths.

Each comparison uses one simulation build and the same seeds. The baseline
includes 76 scenarios at seeds 19 and 41. The 32-bit register profiles also
retain their previous burst-row stress seeds. Each second measurement adds
the two scenarios at both seeds. All profiles use a 64-bit memory bus and
testbench manifest `87c9bc0f0879c09bc53ef0591cf55c39c77b4fa73b6bdd32ae700e62e4478fa5`.

| Channels / register bits | Positive simulations | Line | Condition | Branch | Extracted FSM | Internal toggle | External port toggle |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 / 32 | 200 → 204 | 1,999 → 2,004 / 2,065 | 1,670 → 1,673 / 1,959 | 776 → 779 / 839 | 241 / 470 | 85,596 → 85,919 / 128,968 | 915 → 929 / 1,336 |
| 8 / 32 | 232 → 236 | 3,354 → 3,359 / 3,517 | 3,069 → 3,072 / 3,699 | 1,481 → 1,484 / 1,625 | 937 / 1,838 | 147,894 → 148,253 / 268,278 | 1,022 → 1,036 / 1,388 |
| 2 / 64 | 152 → 156 | 1,999 → 2,004 / 2,065 | 1,671 → 1,674 / 1,959 | 776 → 779 / 839 | 222 / 470 | 87,101 → 88,122 / 128,968 | 1,027 → 1,073 / 1,472 |

The register bank reached 43/43 lines and 19/19 branches in each profile.
All three regressions rejected all nine checker mutations. Two isolated
two-channel RTL mutations also exercised the new checks. Accepting IRQ-enable
writes while reset was busy passed the existing DMA reset test. Accepting
partial word strobes passed the existing register model test. Each new test
rejected its corresponding mutation at both seeds. Only successful simulations
of the unmodified DUT contributed to the coverage tables.

## Whole-DMA regression for address handling and diagnostic events

The regression uses the two- and eight-channel Wait/maintenance RTL snapshots
identified below. Each profile uses one verification build for both measurements.
The baseline includes the previous 72 scenarios at seeds 19 and 41 and the same
additional burst-row stress seeds. The second measurement adds four scenarios at
both seeds. Coverage databases from checker mutations, RTL mutations and other
elaborations stay outside these merges.

Three scenarios exercise descriptor rejection on every channel. Write32 and
Write64 use misaligned addresses, an aligned Write64 and a Fill overflow the
address range, and a Timestamp with a valid target runs with its timer absent.
The checker reads the event through the ring and verifies the command ID,
reason, zero committed bytes, untouched destination and untouched successor
Fence. The seven stopping-error codes exercised by this RTL profile now have a
cross with the physical channel. The eight-channel baseline covered 48/56
pairs and the added cases covered the remaining eight. The corresponding
two-channel cross went from 12/14 to 14/14. The Fence error scenario now
reports its actual opcode to functional coverage.

The fourth scenario gives the command and event rings identical low address
words and distinct upper words. It checks a Copy, its Fence and the error event
from the following rejected command. Truncating AXI read addresses to 32 bits
in an isolated two-channel RTL build let the low-address version pass at both
seeds. The high-address version detected an unauthorized fetch at the low
alias at both seeds. All four scenarios also passed at both seeds on a
one-channel 32-bit memory profile and a three-channel 128-bit memory profile.

Both builds use testbench manifest `715955bcd556e3abdd4c8bea094508e3df49c3bc58d15862f0bc42ba4e9af1fa`. Each arrow shows covered bins before and after adding the four scenarios. Each row retains the same measured denominators.

| Channels | Successful simulations | Line | Condition | Branch | Extracted FSM | Internal toggle | External port toggle |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 192 → 200 | 1,990 → 1,999 / 2,065 | 1,664 → 1,670 / 1,959 | 773 → 776 / 839 | 241 / 470 | 81,712 → 85,596 / 128,968 | 809 → 915 / 1,336 |
| 8 | 224 → 232 | 3,345 → 3,354 / 3,517 | 3,063 → 3,069 / 3,699 | 1,478 → 1,481 / 1,625 | 937 / 1,838 | 139,529 → 147,894 / 268,278 | 879 → 1,022 / 1,388 |

On eight channels the memory interface gained 141 toggle directions and the timer-available input gained both directions. The native hierarchy, port summaries, logic and FSM reports and functional XML were generated from the merged databases. The separate text export failed while rendering individual port toggles. The reporting host also failed a temporary-file allocation probe.

The eight-channel extracted FSM still has 901 unhit transitions. Of those, 896 are the same 14 unhit arcs repeated across 64 burst rows. The maintenance controller accounts for the other five. All arcs remain in the denominator. The remaining logic holes also include discovery capability reads and global IRQ-enable writes during reset. These MMIO behaviors can be checked independently of burst-row transition reachability.

## Wait and context Drain/Abort

The two- and eight-channel RTL design manifests are
`92e072e7110fd3c926df016a8726c186ce1b9ae6b9b65b410b3ab9b991722737` and
`81060ff060bdd03e91b46ce9e3102a2ec50b5473ce55ced3ebe9589f66c71968`.
Both native backends passed 40 tests. The Wait comparator tests cover all 11
conditions at each width, including the signed boundary and modular counter
wrap. The maintenance tests cover wrapped frontiers, later publications,
retained failure bits, and Abort after Resume but before regrant.

The issued-command Abort scenario holds an accepted Fill write response with
an empty command ring. It checks that maintenance stays busy, releases the
response, and verifies the complete destination and successor Fence. The
parked SG scenario checks discard, absence of a completion write, and reuse of
the same channel by a second SG Copy. Nine directed scenarios passed at seeds
19 and 41 on each of five profiles, including 32-bit memory with two- and
three-beat bursts and 128-bit memory with three channels.

The full AXI VIP suite passed 69 scenarios at each of seeds 19 and 41 on
both the two- and eight-channel profiles. Both regressions rejected all nine
checker mutations. The per-channel Wait/Drain scenario also passed at every
channel count from one through eight. It alternates operand width and exercises
equality, signed comparison and counter wrap as the stalled owner changes.
The first eight-channel run hit the Wait yield and resample transitions only
on channel zero. Rotating the stalled owner hit both on all eight, bringing each
channel to 22/22 checked command transitions. The maintenance operation/result
cross reached 6/6.

The final matched testbench added simultaneous faults on the first and last
channel. Drain reported just the first fault seen, then both after the second
channel parked. A request targeting the bound rings replaced that result with
RingBound. Matching Abort returned Success with a clear bitmap and both owners
completed new work. Two independent two-channel RTL mutations verified these
checks at seeds 19 and 41. Completing maintenance as soon as the command ring
was empty triggered the outstanding-Fill check. Retaining only the lowest
failed bit passed the single-fault check, then returned `0x1` when both
channels required `0x3`. Neither mutant contributed coverage.

The extracted maintenance FSM hit nine transitions. Four
remaining arcs return to Idle through hardware reset; the suite applies that
reset only at power-on. The fifth, Busy to RingBound, would require accepting
a new request while ready is low.

| Channels | Positive simulation databases | Line | Condition | Branch | Extracted FSM | Internal toggle | External port toggle |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 186 | 1,990/2,065 | 1,663/1,959 | 773/839 | 241/470 | 81,712/128,968 | 809/1,336 |
| 8 | 218 | 3,345/3,517 | 3,062/3,699 | 1,478/1,625 | 937/1,838 | 139,517/268,278 | 879/1,388 |

Adding reset, replay, write-first fault and Fill seeds hit all 14 observed
burst-row transitions on each of the 16 and 64 physical rows. For example, the
first eight-channel sweep hit Read to Free on 25 rows; the final merge hit it
on 64. The table uses the native whole-DMA hierarchy and external port reports.
The native functional XML and merged per-instance bin data agree on all 22
command transitions for each channel. The separate detailed text export
failed during rendering and awaits a successful rerun.

On the two- and eight-channel default bus profiles, a 16 KiB aligned Copy
transferred 2,048 consecutive read beats and 2,048 consecutive write beats.
The patterned Fill transferred 2,048 consecutive write beats. At seed 19 the
descriptor-to-Fence measurements were 2,113 cycles for Copy and 2,094 for
Fill, identical to the corresponding Write/Fill checkpoint. These are
zero-delay simulation measurements on the 64-bit bus.

Fresh static reviews resolved all 101 CDC and 2,018 RDC findings at two
channels and all 269 CDC and 3,958 RDC findings at eight. Each profile has
689 sink bits checked for absence of downstream fanout in its elaborated
netlist. Six negative controls rejected stale identity, a changed or omitted
endpoint, and invalid grouped-sink evidence. These checks cover the RTL and
the specified asynchronous boundaries. Physical timing is qualified separately.

### Maintenance under contention

Two scenario checks were challenged on a two-channel DUT at seeds 97 and 211.
Using the live producer in place of the captured frontier made Drain hang behind
a later, unrelated Wait on the same ring. Checking just the last channel for
retirement made Drain report success while a lower-numbered channel still held
an unsatisfied Wait. Each mutation changed one RTL predicate. The unmodified DUT
passed the same scenario at both seeds with the same testbench.

The maintenance controller previously hit 25 of 26 extracted conditions. The
missing condition was a valid request presented while Busy. The original test
driver rejected this legal input before it reached the DUT. The driver now
holds the next request until handshake. In the backpressure scenario a blocked
Drain completes successfully, followed by an Abort rejected on a bound ring.
The checker observes the Drain result during the ready cycle before the core
accepts the waiting Abort on the next rising edge.

The 72-case AXI VIP suite passed at seeds 19 and 41 on both main profiles.
Each profile rejected all nine checker mutations. The three new scenarios also
passed for every channel count from one through eight at seeds 19, 41, 97 and
211 (96 runs, including the cases in the full suites). Each of the 32
backpressure runs recorded exactly one Drain/Success and one Abort/RingBound.
With the extra row-reset and replay seeds, the final whole-DMA merges contain
only the successful runs below, all from the updated driver.

| Channels | Runs | Line | Condition | Branch | Extracted FSM | Internal toggle | External port toggle |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 192 | 1,990/2,065 | 1,664/1,959 | 773/839 | 241/470 | 81,712/128,968 | 809/1,336 |
| 8 | 224 | 3,345/3,517 | 3,063/3,699 | 1,478/1,625 | 937/1,838 | 139,529/268,278 | 879/1,388 |

Maintenance reached 26/26 conditions on both profiles. Its extracted FSM
remained at 9/14, and the operation/result cross held at 6/6. Each channel
retained all 22 checked command transitions and every physical burst row hit
all 14 observed row transitions. These counts come from the native summaries,
functional coverage data and FSM reports. The separate detailed text export
remains pending.

### Wait and maintenance timing

The same two design manifests above were synthesized with the Write/Fill
checkpoint's library selection, clock targets and control-budget RAM model.
Both passed all ten mapped crossing budgets. The bus and register setup slack
was 0.0 ps and +307.0 ps respectively in both profiles.

| Channels | Core WNS (ps) | Core TNS (ps) | Violating paths | Logic cell area | Vectorless power (mW) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 0.0 | 0.0 | 0 | 42,055.999 | 78.2983 |
| 8 | −2.4 | −783.6 | 1,109 | 99,482.407 | 162.816 |

The two-channel profile meets the setup target. The eight-channel profile
misses core setup at 1 GHz. A second query against the saved final database
selected one negative-slack core path per endpoint. Its 1,109 distinct endpoints
reproduce both WNS and TNS. Of these, 639 are request/fetch FIFO storage pins,
237 are channel register pins, 124 are command-engine pins, 106 are Copy window
pins and three are channel IRQ outputs. The rising core reset input launches
620 of the FIFO paths, including 439 request FIFO enables and 181 fetch FIFO
data pins. Synchronous reset release is timed by the integration contract.

The worst register request and Copy paths both have −2.4 ps slack. They run
from the core-side register request address to a ring base and from the engine
owner to the next destination address, respectively. Paths ending at the
context maintenance unit's own registers have at least +25.0 ps slack.
Relative to Write/Fill, logic area increased by 1.51% and 3.16%, respectively.
The estimates exclude physical SRAM area, power and intrinsic timing.

## Scalar writes and patterned Fill

The native execution tests drive descriptors for Write32/64, Fence32/64,
Timestamp and Fill. The scalar tests cover target alignment, end overflow, write
errors, reset drain and a fault retry with the external timer changed. The Fill
test compares each destination byte, checks address-space selection and rejects
a read, duplicate byte, crossing burst or out-of-range write. It also exercises
zero length, range overflow and malformed records.

The initial two- and eight-channel Write/Fill/Timestamp snapshots have design
identities `61fd763274a756a17c496fc4156e8ca6747aff115dedda89f71dea60ff76df1f`
and `ef75eca29825839d38eacbd5c55159258709027d5a77558889d50a0b13b5cc93`,
respectively. The implementation profiles constrain the two timer inputs to the
core clock. Fresh CDC/RDC review matched 99/1,874 findings at two channels and
267/3,700 at eight channels, with zero unresolved in each run. The updated
policies drop four former RDC groups for unconnected write-size sinks. Existing
destinations and reset scopes were rechecked, including flattened endpoint checks
for every retained no-clock sink.

Both native simulation backends passed all 36 tests. At two and eight channels,
each full AXI VIP regression passed 60 DUT scenarios at seed 19 and rejected
all nine checker mutations. Focused regressions at seeds 19 and 41 passed on
five configurations. The 32-bit configurations included two-beat and three-beat
bursts. The latter carries 12 bytes per full row, so an eight-byte Fill pattern
must start at a different phase on the next row. The wide configuration used
three channels and a 128-bit bus. The [throughput measurements](../../docs/rtl/performance.md)
include both payload beats and time to the successor Fence.

| Channels | Design manifest | Line | Condition | Branch | Extracted FSM | External port toggle directions |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 2 | `61fd763274a756a1` | 1,925/1,998 | 1,528/1,846 | 762/829 | 205/456 | 789/1,316 |
| 2 | `1e1d33e0eca0b644` | 1,933/2,009 | 1,530/1,856 | 763/831 | 205/456 | 789/1,316 |
| 2 | `a55a0c6ee20fb4ea` | 1,934/2,010 | 1,532/1,858 | 763/831 | 205/456 | 789/1,316 |
| 8 | `ef75eca29825839d` | 3,239/3,408 | 2,563/3,196 | 1,468/1,615 | 771/1,824 | 835/1,344 |
| 8 | `fc4089b82606bdcb` | 3,247/3,419 | 2,565/3,206 | 1,469/1,617 | 771/1,824 | 835/1,344 |
| 8 | `80fa84226d705e22` | 3,248/3,420 | 2,567/3,208 | 1,469/1,617 | 771/1,824 | 835/1,344 |

Logic counts cover the DMA instance hierarchy. External port counts include
both AXI interfaces and the direct ports. The packed timer input contributes
128 measured toggle directions. The simulated counter exercises its low bits;
native Timestamp tests also check an explicit full-width sample and zero.

### Execution descriptor timing

The control-budget profiles use 1 GHz core and bus clocks and a 500 MHz
register clock. At each channel count the runs use identical synthesis
scripts, timing constraints and technology manifests. All runs below
met the bus and register targets and all ten crossing budgets.
Core slack and TNS are in ps. Area and vectorless
power exclude SRAM area, power and intrinsic timing.

| Channels | Descriptor implementation | Design manifest | Core WNS | Core TNS | Violating endpoints | Logic cell area | Power (mW) |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 2 | Per-channel execution context | `61fd763274a756a1` | 0.0 | 0.0 | 0 | 41,861.837 | 78.38 |
| 2 | Shared descriptor at grant | `fdc3b66db5cba943` | 0.0 | 0.0 | 0 | 43,358.589 | 83.21 |
| 2 | Shared descriptor, direct suspend enable | `1e1d33e0eca0b644` | 0.0 | 0.0 | 0 | 41,444.278 | 76.72 |
| 2 | Fill pattern aligned at burst allocation | `a55a0c6ee20fb4ea` | 0.0 | 0.0 | 0 | 41,432.261 | 77.99 |
| 8 | Per-channel execution context | `ef75eca29825839d` | −26.9 | −259,543.3 | 16,714 | 102,903.865 | 170.93 |
| 8 | Shared descriptor at grant | `fed032ff984d6f2a` | −9.3 | −76,164.3 | 13,678 | 106,730.902 | 190.31 |
| 8 | Shared descriptor, direct suspend enable | `fc4089b82606bdcb` | −11.3 | −38,964.4 | 6,754 | 98,500.461 | 165.75 |
| 8 | Fill pattern aligned at burst allocation | `80fa84226d705e22` | 0.0 | 0.0 | 0 | 96,437.846 | 159.46 |
| 8 | Trial with execution data on the fetch request bus | `c28f105dd603d3af` | 0.0 | 0.0 | 0 | 96,971.674 | 164.99 |

At eight channels the original design's worst paths to the walker, burst
window, execution context and request FIFOs launched from the registered
owner. The original's area and power were already 3.94% and 6.05% above
the preceding SG checkpoint. Capturing the descriptor with the grant
improved eight-channel core slack by 17.6 ps. At two channels it cost
3.58% in area and 6.17% in estimated power. With the direct suspend enable,
two-channel area and power are 1.00% and 2.12% below the original.
At eight channels the direct enable reduced area by 4.28% and power by
3.03% relative to the original. Its worst final path ran from the engine
owner through the burst window to the request FIFO's write data.

The first shared-descriptor implementation saved the active command by
decoding the full next phase. The final eight-channel netlist connected one
channel's event-ring capacity register to another channel's suspend slot.
The revised enable comes directly from the scalar page-fault receipt or the
drained Copy/Fill window. In the revised mapped netlists, none of the 64
event producer registers at two channels or the 256 at eight channels have
a path to the suspend bank. The active descriptor data path to the same
bank has +641.6 ps and +742.8 ps slack, respectively. Reset clears
the grant and the channel phases, which invalidate the active and suspended
descriptors respectively.

The first shared-descriptor version and the direct suspend enable version
each passed 60 DUT scenarios at seed 19 and
rejected nine checker mutations at each channel count. Eight replay, fault
and reset scenarios also passed at seeds 1, 7, 97, 211, 503 and 997 on both
profiles for each version. With two Fills parked concurrently, restoring
slot zero for every owner failed the peer destination-byte check. On the
revised RTL, flipping one saved scalar operand bit failed
`WORD_RETRY_RETAINS_TARGET_VALUE`. Both unmodified replay cases passed.
For every channel count from one through eight, seeds 19 and 41 exercised
a faulting Fence and a parked SG Copy on each physical channel. All 144
channel scenarios reached their successor Fence and passed the data checks.
The SG case also ran a peer command during suspension wherever a peer existed.
Each version delivered contiguous 2,048-beat payloads for 16 KiB Copy and
Fill, with 2,113 and 2,094 bus cycles through the successor Fence. The two-
and eight-channel Copy batches took 4,238 and 16,988 cycles.

The first shared-descriptor version had 101/2,068 and 269/4,208 raw CDC/RDC
findings at two and eight channels. For the direct suspend enable, the
corresponding design manifests are `1e1d33e0eca0b644` and
`fc4089b82606bdcb`, with 101/1,891 and 269/3,722 findings. Both versions
passed source-bound review with zero unresolved findings.

The next implementation rotates each Fill pattern when it allocates the
burst row. It stores the result in the 64 bits that hold the source address
for Copy. The two- and eight-channel design manifests are `a55a0c6ee20fb4ea`
and `80fa84226d705e22`. Source-bound CDC/RDC review closed all 101/1,946
and 269/3,892 findings, respectively. Both native backends passed all 36 tests.
Bypassing the rotation failed the native byte comparison.

Final synthesis for commit `34f5e795` met setup at both channel counts.
Core and bus each reported 0.0 ps WNS and zero violating endpoints. The
register clock had +307.0 ps slack. Relative to the initial Write/Fill/Timestamp
implementation, eight-channel logic area fell 6.28% and vectorless power
fell 6.71%. At two channels the reductions were 1.03% and 0.50%.

The eight-channel netlist shows where the rotation moved. Before the change,
the three low destination bits in each of the 64 burst rows reached the upper
320 payload bits in each of the eight execution FIFO entries. After the
change, the destination bits reached the FIFO address pins and had no path to
the sampled payload pins. The stored source-or-pattern field reached the
same payload pins with +521.4 ps slack. The overall core WNS still rounds
to 0.0 ps, including paths to the FIFO, segment walker and window state.

An eight-channel trial also drove the unused data field of fetch requests
directly from the execution port, removing a payload selection at the shared
request interface. It met the same timing targets, with 0.55% more logic area
and 3.47% more estimated power than the committed design. The committed design
retains the existing request interface logic.

A peer Fill that fit entirely in its window could complete from retained row
data even if the engine restored the wrong descriptor. The replay test now
faults the peer's first burst and submits one more burst than the window can
hold. On the two-channel profile at seed 19, forcing channel 1 to restore
channel 0's descriptor corrupted byte 512, the first byte beyond the eight
retained 64-byte rows. The same test passed on the unmodified design.
On both channel counts the strengthened suite passed all 60 DUT tests at
seed 19 and rejected the nine checker mutations. Each aligned 16 KiB Copy
and Fill again sustained 2,048 consecutive payload beats per active AXI data
channel. The measured descriptor-to-Fence intervals were unchanged at 2,113
and 2,094 bus cycles, respectively.

At every supported channel count (1 through 8), seeds 19 and 41 also passed
the patterned Fill, Fill fault/replay and Timestamp ordering cases. These
48 simulation runs checked 59,341 destination bytes against the software
oracle. The patterned Fill case distributes its 12 commands across the
configured channels, and the replay case uses a peer whenever one exists.

The additional patterned Fill, fault, reset and replay seeds bring the merged
totals to 132 distinct passing simulations at two channels and 147 at eight.
Every reserved burst row entered Write directly from Free on a Fill (16/16
and 64/64). All 14 observed row transitions occurred on every row at both
channel counts. The last missing edge at two channels was Reading → Write
on row 1, where a successful read is retained for a later write. The
multichannel reset case at seed 41 exercised it. Combining the individual
simulation hits by row and transition reproduced both merged FSM reports.
The following whole-DMA totals use the full extraction denominator and
exclude rejected checker mutations.

| Channels | Passing simulations | Line | Condition | Branch | Extracted FSM | RTL toggle | External port toggle directions |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 132 | 1,934/2,010 | 1,537/1,858 | 764/831 | 232/456 | 81,062/128,004 | 789/1,316 |
| 8 | 147 | 3,248/3,420 | 2,569/3,208 | 1,469/1,617 | 928/1,824 | 136,628/264,578 | 835/1,344 |

## SG decoder timing cut

The last restored two-channel DMA with SG had core setup slack of −396.4 ps
at 1 GHz. The worst path selected an owner's segment, subtracted the starting
offset, compared the result with the command remainder, then subtracted the
clipped length. Paths ending at the engine grant lock (−395.6 ps) and payload
start (−356.3 ps) also traversed the segment-length comparison. These are
post-optimization results for the earlier SG implementation, before the AXI
receipt-arbitration fix.

The walker timing experiments instantiated the emitted walker with a registered
owner and one registered descriptor per channel. Every run used a 1 ns clock,
50 ps uncertainty and 300 ps input/output delays.

| Channels | Walker | Post-map WNS (ps) | Post-optimization WNS (ps) | Post-optimization TNS (ps) | Violating endpoints after optimization |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2 | Original single-cycle decode | −300.0 | −207.9 | −121,804.2 | 974 |
| 2 | Registered decode | −96.2 | −31.5 | −93.0 | 3 |
| 2 | Registered decode and list address check in Read | −63.1 | 0.0 | 0.0 | 0 |
| 2 | Above with per-channel span selection | 0.0 | 0.0 | 0.0 | 0 |
| 8 | Registered decode and list address check in Read | −117.9 | −52.5 | −17,918.0 | 1,390 |
| 8 | Above with per-channel span selection | −8.8 | 0.0 | 0.0 | 0 |

Capturing the limiting side on entry to Offer leaves register-to-register and
register-to-output WNS at 0.0 ps on both profiles. Input-to-register WNS is
+40.6 ps for two channels and +5.3 ps for eight. At eight channels the mapped
cell area after optimization falls 5.5% with this change, including the same
descriptor registers. Each single-clock wrapper inherited one ineffective reset
exception. Removing it from a restored final database gave zero timing-coverage
diagnostics and reproduced the reported WNS.

Full DMA synthesis uses 1 GHz core and bus clocks and a 500 MHz register clock.
All designs below include the AXI receipt-arbitration fix. The reported slack,
TNS and endpoint counts in this table are for the core clock.

| Channels | SG change | Design manifest | Mapped WNS (ps) | Mapped TNS (ps) | Final WNS (ps) | Final TNS (ps) | Final violating endpoints |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 2 | Baseline | `61f559b1de2d8b29` | −498.2 | −1,545,745.5 | −385.4 | −1,146,998.1 | 12,655 |
| 2 | Decode/Apply split | `845a857e48a7a671` | −73.0 | −288,546.3 | 0.0 | 0.0 | 0 |
| 2 | Split and registered list-address check | `cdb02e9f0f3277b8` | −43.2 | −79,436.7 | 0.0 | 0.0 | 0 |
| 2 | Above with per-channel span selection | `ecfe6a3a55acf3b0` | −42.7 | −83,809.0 | 0.0 | 0.0 | 0 |
| 8 | Baseline | `ebdfd68f862fbe12` | −411.5 | −6,494,254.2 | −305.0 | −2,693,853.0 | 20,536 |
| 8 | Decode/Apply split | `28fdc70060716bdd` | −126.3 | −1,894,887.5 | −29.4 | −144,037.9 | 12,126 |
| 8 | Split and registered list-address check | `14e71f2e2ae43c51` | −101.0 | −1,133,992.3 | −11.4 | −25,453.2 | 4,593 |
| 8 | Above with per-channel span selection | `77ae085572584afe` | −113.2 | −1,301,349.2 | 0.0 | 0.0 | 0 |

The two-channel baseline's worst optimized path and the eight-channel baseline's
worst mapped path both ran from engine owner selection to the SG remaining count.
After the Decode/Apply split, the slowest two-channel mapped path went through
the linear destination base, offset and length checks to control-address state.
At eight channels the worst mapped path crossed the SG walker on its way
from the selected owner to the engine grant lock. Grouping paths by launch
register also found a fetch read-pointer path to the same lock at −126.3 ps.
The worst path launching from command phase was −121.0 ps. This checkpoint
has critical paths independent of the three owner-select bits.
Final optimization met timing in all three domains for all three two-channel
SG changes. The four completed two-channel runs had bus WNS of
0.0 ps, register WNS of +307.0 ps, and passed the ten constrained crossings.
Control-budget logic area was 43,703.894 at baseline, 43,675.804 after the
Decode/Apply split, 42,864.427 after the list-address cut, and 42,057.769
with per-channel span selection (−3.77% from baseline). Vectorless power was
82.30, 82.66, 80.44 and 78.09 mW, respectively.
At eight channels, the split reduced optimized logic area from 102,575.776
to 100,397.335 and vectorless power from 173.74 to 162.20 mW. Both runs
met the bus and register targets and passed the ten constrained crossings.
The worst optimized core path after the split ended at an SG source cursor's
address register. With the registered list-address check the worst path went
from engine owner selection through span length to the SG remaining-count
subtraction on span completion. This run reported 99,776.102 logic area,
161.80 mW vectorless power, and passed the same ten crossings.
The final eight-channel design with per-channel span selection meets setup
in all three domains. Core and bus WNS are 0.0 ps and register WNS is
+307.0 ps. Its logic area is 99,004.834, down 3.48% from the eight-channel
baseline. Vectorless power is 161.18 mW, down 7.23%. The standard flow used
one final optimization pass on both the two- and eight-channel designs.
Restoring each final database reproduced nonnegative setup slack for paths
ending in fetch, the engine, the SG walker, the payload window and the
remaining core registers. Each design passed all ten mapped crossing checks.
The RAM boundary models account for pin loading. Physical SRAM area, power
and intrinsic timing remain outside these pre-layout estimates.

Splitting Decode and Apply reuses the 16-byte entry register and adds one
core cycle per decoded endpoint entry in each traversal. Read now qualifies
the registered list address before issuing a request, using the existing
request cycle. The following results include per-channel span selection and use
RTL design manifests `ecfe6a3a55acf3b0` (2 channels) and `77ae085572584afe`
(8 channels). Each profile passed 50 tests at seeds 19 and 41 and rejected
all nine checker mutations. Only the 100 positive runs entered each full-suite
coverage merge.

| Channels | Command edges | SG edges | Extracted FSM transitions | Line | Branch | Condition | RTL toggle | External port toggle |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 40/40 | 22/22 | 170/456 | 96.62% | 92.12% | 81.80% | 59.73% | 64.08% |
| 8 | 160/160 | 88/88 | 667/1,824 | 95.19% | 90.93% | 78.54% | 46.83% | 66.23% |

The eight-channel stress run added 40 positive cases across eight seeds.
Another 48 seeds exercised window fault/replay. Combining their extracted
transition hits with the main suite reaches 762/1,824 (41.78%). All 64 physical
payload rows took both the read retry and write retry transitions. The new SG
cases checked index exhaustion and error priority, table-address overflow and
alignment, overflow of the used payload interval, an empty fragment at the
address limit and a valid clipped tail. Both tests also passed on one 32-bit
memory channel and three 128-bit memory channels. All 35 native RTL tests
passed on both simulation backends.

Two isolated DUT mutations on the list-address cut checked that scenario before
the span-selection change. Allowing
an invalid metadata request to issue triggered the control-tag double-accept
assertion. Reporting an exhausted list as InvalidLength triggered the event
checker, which expected LengthMismatch. Repeating each mutation at seed 41
reproduced its first failure.

The parked-span scenario faults the first write while the source segment is
shorter than the destination segment. A peer completes a segmented Copy,
then the owner resumes. Software checks the saved bytes, remaining spans and
final Fence on each owner. Changing the selector to one shared bit caused an
unauthorized read at seeds 7 and 23. An independent simulation compared both
walkers on every clock edge of the eight-channel DMA for 20 runs. All cursor
states, metadata accesses and qualified span outputs agreed through peer
execution, fault and reset.

At both main seeds the linear throughput measurements matched the earlier RTL
cycle for cycle. An aligned 16 KiB transfer sustained one 64-bit beat per bus
cycle on both R and W over 2,048 beats, with 2,113 cycles from descriptor to
Fence. The eight-channel batch at seed 41 took 16,988 cycles from the first
descriptor to the last Fence for 128 KiB. These measurements exercise the
linear fast path.

The one-channel native CopyEngine bench also measured a 24-byte transfer
with a fixed memory responder. From command acceptance to the final write
receipt, linear/linear took 8 core cycles before and after the change.
With one SG endpoint the count rose from 55 to 61 cycles, and with two SG
endpoints it rose from 88 to 96. The corresponding counts of memory
requests stayed at 2, 12 and 22. The SG cost falls on the entries decoded
during validation and execution.

At seed 73, configurations with 4, 5, 6 and 7 channels each passed seven
cases covering SG boundaries and recovery, mixed Copy, reset with reordered
write responses, and multichannel throughput. The parked-span case also passed
on the one-channel 32-bit and three-channel 128-bit memory profiles. A separate
eight-channel run repeated the zero-delay 128 KiB batch and the withheld-B
reset case at 16 seeds. All 32 runs passed. The batch took 16,988 bus cycles
at every seed.

The source-bound crossing reviews left zero unresolved findings. The two-channel
review covered 99 CDC and 1,804 RDC findings, and the eight-channel review
covered 267 CDC and 3,490 RDC findings. The SG change removed two temporary
decoded payload fields from the unconnected-sink report. The connectivity
query verified zero downstream sequential endpoints for every remaining
reported unconnected sink bit on the corresponding elaborated design.

### Reset of a parked burst window

Testbench manifest `74e63c28bccdbde7` adds a second recovery path to the
window replay scenario. Software faults a read or write after the first
window, observes FaultStatus, then resets the channel. It reconfigures the
same rings and checks a fresh Copy that wraps the window, including its
entire payload and successor Fence. Seeds 19 and 41 passed all 51 tests
on both the two- and eight-channel RTL above. Each profile rejected the
nine checker mutations.

The extracted FSM coverage reached 184/456 on two channels and 739/1,824
on eight at the two main seeds. Twenty-four extra reset seeds on eight
channels brought the union to 853/1,824. Every one of the 64 physical rows
transitioned from Read to Free and from Write to Free. Running the 40-case
stress set on the same simulation build raised the union to 864/1,824
(47.37%). All 13 observed row transitions were hit on all 64 rows, and the
four error-code transitions were hit on every channel. The extraction
denominator includes the unobserved edges. The one-channel
32-bit and three-channel 128-bit memory profiles also passed both the
reset and resume cases. In a two-channel DUT mutation, keeping the fault
latch through reset and command start let the ordinary resume test pass.
The reset case detected the missing Fence for the replacement command at
both seeds 19 and 73.

The remaining 15 unhit transitions were the same on every row. Ten
conflict with the read-issue and receipt predicates described below. Of the
other five, three enter Writing from Free, Read or Done. A forwarded write
requires a successful Reading receipt, and a buffered write requires a
prepared RAM read for that row. For Reading → Free, channel reset must first
drain its outstanding read. Done → Read would require retiring a previously
completed head and reallocating that same row on the same edge. In the
170-run checkpoint below, the long and mixed Copy cases exercised same-edge
reuse as Writing → Read. Only nine test classes entered Done, all during
reset, payload error or window replay. The union of individual simulation
hits matched the merged report by row and transition. Reachability of
Done → Read remains open. All 28 transitions per row remain in the
coverage denominator.

### SG error progress across spans

Testbench manifest `e0c6a834503563f2` adds a read error on the first beat
of a second SG span and a write error after 4 KiB of that span. The first
event must report one completed beat. The second must report that beat plus
4 KiB. On each channel the checker compares every destination byte in those
prefixes and checks the event identity and code. At seeds 7 and 73 this
accounted for 4,104 bytes per run on one 32-bit channel, 8,224 on two 64-bit
channels, 12,384 on three 128-bit channels, and 32,896 on eight 64-bit
channels. Two independent RTL mutations omitted either the earlier span or
the current span's committed progress. Each failed the event check at both
seeds.

The two main seeds passed all 52 tests and rejected nine checker mutations
on each of the two- and eight-channel profiles. The whole-DMA extracted FSM
coverage was 185/456 and 745/1,824, respectively, before adding stress seeds.
The combined eight-channel report retains 168 distinct passing simulations
from that build, including 104 main, 24 reset and 40 mixed stress runs. It reports
95.19% line, 90.93% branch, 78.64% condition, 47.15% RTL toggle and 66.23%
external port toggle. Its 864/1,824 FSM transitions match the independent
union by row and edge. Rejected mutations were excluded from this merge.

### Command ring admission

Testbench `8ba42bbc064004b1` checks the AXI4-Lite error response and
enable readback for each invalid command ring geometry in the scenario.
A valid Copy and Fence on the same channel then verify that the rejected
enable left the ring configurable. All 53 tests passed at seeds 19 and 41
on both the two- and eight-channel designs. Each profile rejected nine
checker mutations. The same admission test passed on three channels using
64-bit AXI4-Lite at seeds 7 and 73. Removing the command ring's extent
overflow check caused both seeds to fail on a two-entry ring based at the
last aligned address.

The eight-channel merge for this build contains 170 distinct passing runs
(106 main, 24 reset and 40 mixed stress). The command geometry cases exercised
56 additional condition bins and 8,342 additional RTL toggle bins compared
with the 168-run checkpoint. Whole-DMA coverage reached 80.44% condition and
50.16% RTL toggle. Line remained at 95.19%, branch at 90.93%, external port
toggle at 66.23%, and the extracted FSM at 864/1,824 (47.37%). Each of the
eight channels exercised all 20 command and 11 SG phase transitions.

To reproduce that merge, use the same eight-channel simulation build for
all three invocations of `flow/regression.sh`. Merge only passing DUT runs.

| Test selection | Seeds |
| --- | --- |
| All 53 cases in `flow/tests.list` | 19, 41 |
| `pdma_window_reset_test` | 1, 3, 5, 7, 11, 13, 17, 23, 29, 31, 37, 43, 47, 53, 59, 61, 67, 71, 79, 83, 89, 97, 101, 103 |
| `pdma_mixed_copy_test`, `pdma_sg_range_test`, `pdma_sg_recovery_test`, `pdma_reset_reordered_b_test`, `pdma_window_replay_test` | 1, 7, 11, 23, 53, 97, 211, 503 |

A separate eight-channel build varied synchronizer sampling latency by zero
or one destination cycle. Seeds 7, 41 and 73 each passed eight scenarios
covering SG park/replay, cross-span error progress, reset with a withheld B,
multichannel IRQ and linear throughput. All nine checker mutations were
rejected. Over 2,048 beats the 16 KiB copy had zero read gaps across all six
throughput runs. Total write gaps were 0–1 bus cycles with equal clocks and
1–2 with the core at half the bus frequency. This simulation-only build has
its own results, separate from the production RTL coverage above.

## Execution state coverage

At `4dc13e46` the automatic FSM report covers two families in `CopyWindow`.
Each retained payload row contributes 28 candidate edges. Each channel also
contributes four error-code edges (enter/clear a permanent read or write error).
The same 44 tests and seeds 19 and 41 give the following breakdown.

| Channels | Row state values hit | Row transitions hit | Error-code transitions hit | Total transitions hit |
| ---: | ---: | ---: | ---: | ---: |
| 2 | 80/96 | 117/448 | 4/8 | 121/456 |
| 8 | 274/384 | 360/1,792 | 4/32 | 364/1,824 |

Two effects account for the low score. For example, the eight-channel report
counts ten edges per row that conflict with the row update guards. Only a
row in Read can issue a read and become Reading. Only a successful response
to Writing can produce Done. Only a receipt for Reading or Writing can set
Write. The report includes four, three and three incompatible source states
respectively for those destinations, spanning 640 candidate edges across the
64 rows. All remain in the native denominator.

There are also real stimulus gaps. At this checkpoint all 64 rows exercised
read issue, but only two exercised the Reading-to-Read retry edge. Only channel
zero exercised the permanent payload read/write errors. The expanded cases
target those errors on every channel and replay reads and writes after a full
window of payload. The independent command and SG phase covergroups record
per-owner transitions that native extraction misses in packed contexts.

At seed 7 the two- and eight-channel profiles both passed the permanent Fence
write error, invalid event-ring geometry and window replay scenarios. Window
replay checked 4,352 and 17,408 payload bytes respectively. A separate SG
fault/resume/reset run on each profile hit 8/20 command edges and 9/10 SG
edges across the design. That scenario uses two SG endpoints, so it did not
exercise the walker's linear-endpoint decode edge. These focused runs verify
the new probes; the table above remains the earlier full-suite baseline.

At `67b980e8` the 47-test suite passed at seeds 19 and 41 on each profile.
The extracted FSM reached 168/456 transitions on two channels and 663/1,824
on eight. The permanent read/write error-code edges were covered on every
channel. The command covergroup reached 20/20 across the design, but only
channel zero hit all 20. Each peer missed Fence fault, Fence resume, SG list
error and publication of the second fragment of an event. All channels
hit the 10 selected SG walker edges.

The per-channel regression places each channel's first event across its own
4 KiB boundary. It checks the error's identity and code, one consumer advance,
and the untouched adjacent slot. The SG validation and Fence fault/resume
scenarios also run on each owner. The W-before-AW test checks the ordering
observation separately for each faulting Fence.

With testbench manifest `28ffa369e73dc00b` and the same two seeds, both RTL
profiles passed all 94 runs and rejected all nine checker mutations per profile.
The command and SG columns sum the bins **per channel**. The remaining columns
use the whole-DMA metrics defined in the verification README.

| Channels | Command edges | SG edges | Extracted FSM transitions | Line | Branch | Condition | RTL toggle | External port toggle |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 40/40 | 20/20 | 170/456 (37.28%) | 96.50% | 91.83% | 81.40% | 56.05% | 64.08% |
| 8 | 160/160 | 80/80 | 667/1,824 (36.57%) | 95.09% | 90.72% | 78.19% | 42.12% | 66.23% |

Eight additional seeds (1, 7, 11, 23, 53, 97, 211, 503) ran the eight-channel
window replay scenario. Taking the union of hits by extracted machine and edge
adds 71 transitions to the two-seed result, reaching 738/1,824 (40.46%).
Read retry (`Reading → Read`) reached 55/64 rows and write retry
(`Writing → Write`) reached 50/64. Each run checked payload data after Resume.
The native transition denominator is unchanged throughout.

## Mixed Copy traffic and channel configuration lifetime

Each mixed run submits eight checked tasks per physical channel. Each channel
uses every Linear/SG endpoint pair with Fence polling and with a Trap after the
Fence. The order, source and destination split points, contexts and AXI delays
vary with the seed. The register case separately verifies that disabling a
configured channel retains its ring-context lock. It checks the actual write
error, retained value, reset clearing and a subsequent DMA fetch using a
replacement context on every channel.

At seeds 1, 97 and 211 the mixed runs also measured memory-pin handshakes.
Across those three runs, two channels produced 95 RLAST overtakes and 81
switches of RID after a nonterminal beat. Eight channels produced 662 and 588
respectively. Both profiles reached eight outstanding reads. The mixed runs
saw zero B overtakes. The independent reset case at seed 41 held one
channel's B and observed two peer B overtakes on each profile before releasing
the held response. The peer Fence, target reset-busy state and eventual reset
completion were checked in that case.

The first run with separate IRQ service coverage reached 3/3 result bins
across the design. The per-instance report showed only channel zero had
serviced a fatal interrupt. The fatal tests now trigger command-read and
event-write failures on each channel in turn and require a healthy peer to
finish before software resets the failed channel. Group detail is generated
for fully covered group types as well, so a missing instance bin stays visible
even when the type-level percentage reaches 100%.

The final testbench manifest starts with `04570a4cfcec9f76`. Against the
same RTL snapshots as the AXI receipt checkpoint below, all 44 DUT tests
passed at seeds 19 and 41 on each profile. Each coverage merge contains 88
passing runs. Each profile also rejected all nine checker mutations.

| Channels | Checked endpoint pair × completion path × channel | IRQ results across instances | Line | Branch | Condition | Extracted FSM transitions | RTL toggle | External port toggle |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 16/16 | 6/6 | 96.14% | 91.34% | 80.82% | 26.54% | 54.86% | 64.08% |
| 8 | 64/64 | 24/24 | 94.88% | 90.46% | 77.18% | 19.96% | 41.95% | 66.23% |

The IRQ counts require all three checked service results on each physical
channel. On eight channels the control/register subtree's branch coverage
increased from 85.14% to 87.74%. Extracted FSM coverage stayed at 364/1,824
transitions. Explicit coverage of command and SG lifecycle transitions across
stall, fault, resume and reset remains open.

## AXI read/write receipt arbitration

The eight-channel run at seed 41 exposed a one-cycle delay between a payload
RLAST and its dependent write. At burst index 29 the endpoint accepted RLAST
and B on the same edge. Its previous uncontended read grant had given the next
tie to B. The read receipt waited a cycle before crossing to the core. With
continuous traffic thereafter the write stream could sustain one beat per
cycle, but could never recover that cycle. At seed 11 an immediately following
descriptor RLAST also saw RVALID high and RREADY low while the B won the grant.

The merger now records when a B actually waits behind a read. An initial tie
forwards R and a waiting B takes the next available grant, including after a
downstream stall. `ReceiptPriorityTest` reproduced the dropped RREADY cycle
on the previous RTL. As a sensitivity check, unconditional read priority
failed its deferred-write assertion. All 35 native tests passed on CC (stride
1 validation) and Cranelift with the fix.

On the corrected eight-channel DUT, burst 29 at seed 41 went from 13 to 12
bus cycles between its last R and first W handshake. At seed 11 the blocked
RLAST cycle disappeared. The original write budget and Fence/data checks were
unchanged. Of 11 matched throughput seeds (1, 7, 11, 19, 23, 41, 53, 97, 211,
503, 997), five had failed before the fix (1, 11, 23, 41, 97). All 11 passed
afterward. In each run channel 0 transferred 2,048 beats in 2,216 cycles per
direction with 168 peer descriptor R beats. All eight tasks passed the memory
and Fence checks, and their 128 KiB batch took 16,988 bus cycles from first
descriptor to last Fence.

Both profiles passed all 42 DUT scenarios at seeds 19 and 41 and rejected all
nine checker mutations per profile. The whole-DMA instance reports each merge
84 passing runs. The design digests below identify the corrected snapshots.

| Channels | Design digest (prefix) | Line | Branch | Condition | Extracted FSM transitions | RTL toggle | External port toggle |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | `61f559b1de2d8b29` | 95.94% | 90.97% | 80.30% | 26.54% | 53.05% | 62.90% |
| 8 | `ebdfd68f862fbe12` | 93.85% | 88.27% | 75.14% | 19.96% | 38.77% | 65.07% |

## Whole-DMA coverage at `a26e1f99`

The 32-bit register / 64-bit memory profiles ran the same 42 DUT tests at
seeds 19 and 41. The two-channel regression passed 84/84. The eight-channel
regression passed 83/84 and failed one throughput check. Each profile also
rejected all nine checker mutations. Only passing DUT runs contributed
coverage, with a separate native merge per elaboration.

| Metric (covered / measured) | 2 channels, 84 runs | 8 channels, 83 passing runs |
| --- | ---: | ---: |
| Line | 1,890 / 1,970 (95.94%) | 3,172 / 3,380 (93.85%) |
| Branch | 735 / 808 (90.97%) | 1,407 / 1,594 (88.27%) |
| Condition | 1,385 / 1,728 (80.15%) | 2,307 / 3,078 (74.95%) |
| Extracted FSM transitions | 121 / 456 (26.54%) | 364 / 1,824 (19.96%) |
| Toggle across the RTL instance tree | 70,044 / 132,052 (53.04%) | 107,692 / 277,830 (38.76%) |
| External DMA port toggle | 746 / 1,186 (62.90%) | 790 / 1,214 (65.07%) |
| Current scenario covergroup | 18 / 19 bins | 18 / 19 bins |

RTL counts span the AXI-Lite frontend, per-channel registers, command fetch
and arbitration, shared execution, CDC transport and memory bus endpoint,
including their shared-cell instances. External port counts combine direct
top ports, register bus signals and memory bus signals. For eight channels
these contribute 29/44, 182/312 and 579/858 toggle directions respectively.
Unused port bits remain in the denominator. The [report guide](README.md)
describes the instance scope and how to locate missing bins.

| Eight-channel RTL instance | Line | Branch | Condition | Toggle |
| --- | ---: | ---: | ---: | ---: |
| AXI-Lite frontend | 97.80% | 90.62% | 82.18% | 41.43% |
| Control and channel registers | 89.36% | 85.14% | 66.42% | 19.70% |
| Commands and execution | 95.90% | 90.50% | 78.59% | 30.98% |
| CDC transport | 100.00% | 100.00% | 98.35% | 90.82% |
| Memory bus endpoint | 96.99% | 89.26% | 87.18% | 82.02% |

The eight-channel `pdma_multichannel_throughput_test` at seed 41 observed
2,048 payload beats in each direction. There were 168 read gaps, 169 write
gaps, 168 competing descriptor beats and zero additional write-cycle allowance.
An isolated replay at this revision and at the pre-SG revision `9f2ee362`
reproduced the same counts and failure. The receipt arbitration fix above
closes this failure on subsequent RTL.

Three gaps matter for subsequent regression work.

- One unexercised control case is changing a locked channel's ring
  context. A frontdoor check can verify the access error and the retained value.
- Native FSM extraction found only `CopyWindow` state variables. It saw
  84/102 states with two channels and 284/408 with eight. Command and SG
  phases held in packed contexts need explicit transition coverage.
- `IRQ_NOTIFICATION` was the sole missing scenario bin. Software checks the
  interrupt cause and resolves a successful Trap/Fence to a task success
  before that collector samples. Channel IRQ service needs its own observation
  point. Directed scenarios passed for ring wrap, peer IRQ, global reset and
  a peer write response overtaking a retained response during reset. The 19
  task bins do not measure combinations of these behaviors. See the
  [whole-DMA coverage requirements](requirements.md#whole-dma-coverage-review).

## Independent scatter/gather Copy

With the upgraded compiler at PDMA checkpoint `9f2ee362`, the pre-SG
two-channel snapshot passed review of 99 CDC and 1,584 RDC findings. The
eight-channel snapshot passed review of 267 CDC and 3,054 RDC findings.
Each pass used its own RTL identity and flattened endpoint evidence.

The SG RTL passed 34 native tests on each of CC (validation stride 1) and
Cranelift at seed 23. The direct engine tests exercise independent list starts,
four endpoint combinations, a permanent write error after 16 committed bytes,
a zero-length SG command, and channel reset with an accepted list read still
outstanding. The late failed list receipt permitted channel cleanup and a fresh
Trap. The model checks passed for SG copies (3), range validation (3),
SG faults (4), multichannel SG reset (1) and single-channel scenarios (3).

With the 64-bit memory port at seed 19, the 2- and 8-channel UVM suites each
passed all 42 DUT tests and rejected nine checker mutations with their expected
diagnostics. In both configurations all three SG tests also passed at seeds
11 and 97 with independently randomized clock timing. They further passed for a
1-channel 32-bit memory port (one slot, two beats) and a 3-channel 128-bit port
at seed 41. The source and destination place their logical first fragment at
different physical offsets. The memory monitor denies access to unselected
table entries. A late destination validation error permits no payload read or
write. A faulting payload write retains its bytes across source mutation;
a peer can complete before the owner resumes or resets.

For both 64-bit configurations the zero-delay long, short, batch, clock-ratio
and multichannel throughput measurements matched the pre-SG toolchain baseline
at the same seed. An aligned 16 KiB linear Copy on one active channel took
2,048 beats and 2,048 bus cycles per payload direction. First descriptor AR to
Fence B took 2,113 cycles. Four 64-byte Copy/Fence pairs took 285 cycles.
These measurements cover the linear path. SG additionally pays for list
validation, traversal and span boundaries.

| SG configuration | Design digest (prefix) | Emitted RTL digest (prefix) | CDC reviewed | RDC reviewed | Unresolved CDC / RDC |
| --- | --- | --- | ---: | ---: | ---: |
| 2 channels | `9ce3260ae1e8f610` | `a1034f07b4c90645` | 99 | 2,067 | 0 / 0 |
| 8 channels | `275843cda5db6573` | `810e3035e4935562` | 267 | 4,080 | 0 / 0 |

The walker introduces no clock crossing. RDC reports additional paths to the
three 64-bit fields of its local decoded cursor and changes the path counts
for existing sinks. In each configuration a flattened connectivity query
confirmed zero downstream sequential endpoints for every one of the 817 bit
selections reported as unconnected. The review matches the exact endpoint,
source reset, destination clock classification and path count for each group.

### SG synthesis at `a26e1f99`

Both control-budget runs on the SG snapshots above reached their five-hour
limit. The two-channel run saved the post-optimization database. Restoring it
reproduced the clock-group results below. The eight-channel run was still in
mapping and produced no mapped checkpoint.

| Two-channel clock | Target period | Worst setup slack | Violating paths |
| --- | ---: | ---: | ---: |
| Core | 1 ns | −396.4 ps | 10,361 |
| Memory bus | 1 ns | 0.0 ps | 0 |
| Register bus | 2 ns | +307.0 ps | 0 |

The worst paths start at the registered execution owner and end in the
per-channel SG walk state, including `remaining`. Core total negative slack
is −1,179,930.6 ps. The mapped-cell library audit passed. Logic cell area was
43,670.385 in the library's area units. The restored database gave a vectorless power estimate of
81.79 mW and passed the mapped crossing checks for 10 mailbox/FIFO paths, five
held-data paths and 10 synchronizer chains. Both estimates exclude the
8,192-bit payload SRAM; its area, power and intrinsic timing remain open.
These results precede the AXI receipt arbitration change at `4dc13e46`.

## Active-high interrupt lines

On the two-channel DUT at seed 41, the stock JarvisUK level monitors verified
six directed cases. Unmask or enable exposed a retained Trap immediately, W1C
lowered its line, and two completed batches shared one IRQ while their handler
was blocked. Channel reset cleared an asserted Trap. Enabling IRQ on a stopped
channel exposed the error after software had already consumed the event.
A blocked channel handler left its peer operational, and a device fatal reached
its own handler. Native Veryl tests passed on both CC and Cranelift (32 each).
The device-fatal test observed the asserted level for twelve additional core
cycles. The earlier delivery-handshake evidence below describes its own revision.

At `11508d89`, the 2- and 8-channel seed 19 runs each passed all 38 DUT
scenarios and rejected all eight checker mutations with their expected diagnostic.
Fresh snapshots reproduced all 49 compiled SystemVerilog files per configuration.
With zero AXI delay and one active channel, both configurations transferred an
aligned 16 KiB Copy at 8 bytes per bus cycle in each direction. Each payload
stream used 2,048 cycles with zero bubbles. Descriptor to Fence took 2,113
cycles, and four prepublished 64-byte Copy/Fence pairs took 285 cycles from
first descriptor to last Fence. These match the earlier handshake checkpoint.

The CDC and RDC checks on the regenerated snapshots completed as follows.

| Channels | CDC findings reviewed | RDC findings reviewed | Unresolved CDC / RDC |
| ---: | ---: | ---: | ---: |
| 2 | 99 | 1,584 | 0 / 0 |
| 8 | 267 | 3,054 | 0 / 0 |

Each channel IRQ and the fatal IRQ are outputs timed against `core_clk`. The
port constraints follow the removal of the ready inputs. Each check first
rejected the prior review's source identity, then passed with the same reviewed
endpoints and reasons bound to the current snapshot. In each configuration,
a fanout query on the elaborated design confirmed zero downstream sequential
endpoints for all 625 bit selections in the unconnected RDC groups. The external
interrupt controller and its clock crossing are outside this DMA boundary.

Both configurations met the pre-route setup targets of 1 GHz for core and bus
and 500 MHz for registers. The first eight-channel run timed out during
mapping. A fresh run completed from the same snapshot with the same synthesis
and timing settings and a longer wall-clock budget. Each configuration passed
all ten crossing, five held-payload and ten local synchronizer timing checks.
The control-budget area and vectorless power estimates exclude physical Copy SRAM.

| Channels | Worst setup slack (ps) | Worst channel IRQ slack (ps) | Fatal IRQ slack (ps) | Mapped cell area | Vectorless power (mW) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 0.0 | 69.8 | 403.5 | 38,460.174 | 73.211 |
| 8 | 0.0 | 0.0 | 336.2 | 90,445.824 | 147.636 |

In both configurations the worst path to each channel IRQ starts at its event
ring's producer or consumer pointer. At two channels the output margins were
69.8 and 87.2 ps, compared with 446.9 ps on each registered handshake output.
At eight channels the tightest paths ran from the channel 3 consumer pointer
to `irq[3]` and the channel 4 producer pointer to `irq[4]`. Both reported
0.0 ps of slack, and `irq[1]` had 0.1 ps. All three clock groups had zero
setup violations. These IRQ paths warrant a post-route check.

The two-channel Trap handshake baseline measured 38,443.733 in cell area
and 73.378 mW. The available eight-channel baseline predates Trap and measured
86,309.130 and 141.226 mW. Its deltas of +4.79% in area and +4.54% in power
cover both the addition of Trap and the change to level IRQs.

## Earlier per-source service on the superseded handshake interface

With one shared IRQ service, holding channel zero's software event-consumer lock
caused channel one's IRQ wait to time out at seed 41. Both channels had already
published their error events. With one service per source, the same test passed
on the two- and eight-channel builds. Every peer consumed its event through
its own ISR before the test released channel zero.

Two further scenarios passed at one, two and eight channels with the same seed. One accepted
two Trap notifications on a single channel while its first handler was blocked,
then checked both Fence markers and both payloads. The other injected a reserved
memory response reason on a descriptor fetch. It observed the global fatal
register and exactly one device-fatal ISR while the channel Trap ISR was blocked.

At seed 19 the two- and eight-channel builds each passed all 38 DUT scenarios
and five TB selftests. Each build rejected all eight checker mutations with
their expected diagnostics.

## Earlier Trap execution and notification-handshake checkpoint

The Rust system scenario submits Copy, Fence and Trap, observes one channel interrupt,
and checks the Fence and copied data without consuming an event. The six model IRQ
tests also pass. In the native RTL test the next Trap is already at the head while the
preceding Fence write response is withheld. The original RTL failed at
`TRAP_EXECUTED_AT_ACCEPTANCE` after the response was released. The updated RTL
passes that check, rejects a Trap with nonzero reserved bytes, and retains a new Trap
when its cause is acknowledged on the same core edge. CC and Cranelift each ran
32 native DMA tests with no failures.

The interrupt scenario publishes a three-command batch on every active channel.
Its watchdog allows one Copy service budget per competing channel. With an
unscaled budget the eight-channel seed 19 run expired while the shared engine
was still issuing byte transfers for a peer, before channel zero issued its Fence.
The same seed reproduced all 650 AXI read/write address issues up to the original
timeout (direction, ID, address and size), then issued channel zero's Fence32.
With the per-channel budget it checked 16 tasks and 1,024 payload bytes
across the two rounds. Each round required an interrupt from every channel.
At seed 41 both configurations also passed all three directed Trap scenarios.
They check delivery with the event mask set, sticky cause across repeated delivery,
no replay on unmask/enable, W1C with a notification pending, coalescing under
IRQ backpressure, and channel-reset clearing.

At seed 19 each configuration passed all 36 DUT scenarios. Both builds also
rejected all eight checker mutations with their expected diagnostics.

The aligned 16 KiB Copy on each configuration took 2,048 bus cycles
in each data direction, with zero bubbles and 2,113 cycles from descriptor to
Fence. Four prepublished 64-byte Copy/Fence pairs took 285 cycles through the
last Fence. Both workloads retained the prefetch checkpoint's cycle counts.

The two- and eight-channel CDC/RDC reviews covered the new RTL snapshots. They
reviewed respectively 99/1,584 and 267/3,054 CDC/RDC findings, with zero unresolved.
The rules and endpoints matched the prefetch checkpoint. Only the source identities
in the two review files changed.

## Channel-owned prefetch and DMA-owned AXI master

The AXI4-Lite register frontend ships in `common_cell 0.2.0`. DMA owns descriptor reservations, AXI memory identity, burst assembly, tracking and lifecycle. PDMA and `tech_cell_generic 0.1.1` resolve the same published common-cell revision. Resolving from an empty dependency cache and without a lockfile reproduced the committed PDMA lockfile.

At default FIFO depth two, the CC and Cranelift backends each passed 31 DMA tests, 12 shared-cell tests and three generic RAM provider tests. Fresh two- and eight-channel builds using the published dependencies each reproduced all 49 SystemVerilog compilation units byte for byte from the November 20 candidate, with identical profile settings, constraints and passive checkers. Directed tests withhold all reads until two descriptor rows are reserved, interleave data beats, reverse B completions, stall AR across a priority change, and reset one channel with two fetch segments outstanding while its peer completes a Fence.

Each 2- and 8-channel configuration passed all 33 DUT scenarios at seed 19 and rejected all eight checker mutations with their expected diagnostics. The multichannel Copy, park, channel reset and global-reset-peer scenarios also passed seeds 7 and 41 on both configurations. The standalone SV Copy/Fence/reset smoke passed on the default profile. All eight channel counts elaborated and ran the multichannel Copy scenario. The one-channel 32-bit profile at depth one and the three-channel 128-bit AXI4 profile at depth three each ran the four focused multichannel scenarios and rejected all eight checker mutations.

With random VIP delays at seed 19, the multichannel Copy scenario reached eight outstanding ARs on both configurations. The 2-channel case recorded 13 out-of-order read completions and seven RID changes within unfinished bursts. At 8 channels these counts were 129 and 89. Maximum outstanding AW was four and seven respectively. These scenarios observed zero B overtakes. A separate DUT scenario holds one old write response across channel reset. A peer completes its Copy and publishes its Fence while that B is still withheld. The reset-busy bit stays asserted until the old B is returned. Both 2 and 8 channels observed two B overtakes at seeds 19 and 41. The AXI endpoint's native test independently checks the returned tags.

Fresh CDC/RDC runs against the published two-channel snapshot reviewed 99 CDC and 1,584 RDC findings. At eight channels the counts were 267 and 3,054. All four runs had zero unresolved findings. Only the source identity changed in the review files. Their endpoint lists, including the unconnected RDC sinks checked bit by bit for zero fanout, and their acceptance criteria stayed the same.

The prefetch control-budget synthesis met the 1 GHz core and bus and 500 MHz
register setup targets at both two and eight channels, with 0 ps worst slack.
At two channels, mapped cell area rose from 29,500.674 to 38,980.067 (+32.1%)
against the November 19 default two-channel profile. Vectorless power rose from
58.32 to 73.74 mW (+26.4%). At eight channels the mapped area was 86,309.130
and vectorless power was 141.23 mW. These figures exclude the physical Copy
SRAM. Workload power and routed timing remain unqualified.

See [the measured workload comparison](../../docs/rtl/performance.md) for payload utilization, batch latency and the depth-one comparison.

## Earlier shared-endpoint experiment

The preceding local review candidate moved both endpoints, their tests and the passive
ready/valid property into `common_cell`. The published 0.1.0 package predates
these endpoints. These checks used local package sources. Updating the native
lockfile after release also requires a compatible `tech_cell_generic` dependency.

Both native simulators passed 16 shared-cell tests and 24 DMA tests each.
The new ordering test admits two two-beat reads followed by a write, returns
the write first and stalls its receipt. Both read receipts subsequently
deliver their original tags and complete data in request order.

Across 15 integration profiles, all 49 generated SystemVerilog compilation
units per profile matched the preceding endpoint checkpoint after normalizing
the package prefix and ignoring comments and whitespace. The profile inputs
matched byte for byte. The mapped synthesis results below are from the
preceding checkpoint.

Fresh DUT simulations on the two-channel, eight-channel and two-channel AXI4
profiles passed 12 scenarios and detected all 24 injected checker failures.
Each zero-delay 16 KiB Copy transferred 2,048 read and 2,048 write beats in
2,048 bus cycles per direction, with 2,161 cycles from descriptor to Fence.
The two- and eight-channel CDC/RDC reviews reproduced the counts in the next
section, with zero unresolved findings. The standalone mailbox CDC check
reported zero findings.

## AXI endpoint extraction — local review candidate

The local `bus_cell` candidate was exercised with the DMA core and with an
independent consumer. The latter carried a high 48-bit register address,
48-bit memory address, six USER bits, an 11-bit tag and 128-bit read data
through emitted SystemVerilog. Both native simulators passed the four endpoint
tests and 24 DMA tests. An isolated checker test held a transaction stalled and
detected a deliberate change to its payload.

| DUT regression | Positive runs | Expected checker rejections |
| --- | ---: | ---: |
| 14 configurations from the preceding checkpoint | 448 | 112 |
| Default two-channel profile with the AXI4 memory port | 32 | 8 |
| Eight-channel synchronizer-delay stress | 16 | 8 |

The shared endpoint preserved the zero-delay 16 KiB Copy measurement on both
default channel counts and on the AXI4 variant: 2,048 cycles for 2,048 data
beats in each direction, and 2,161 bus cycles from descriptor to Fence. The
eight-channel build reproduced all 14 previously measured latency distributions
at the same seed. All 256 eight-channel positive runs contributed to the merged
coverage database, which hit all 18 PDMA scenario bins.

| Channels | CDC raw / reviewed / unresolved | RDC raw / reviewed / unresolved |
| ---: | ---: | ---: |
| 2 | 66 / 66 / 0 | 1,517 / 1,517 / 0 |
| 8 | 210 / 210 / 0 | 2,369 / 2,369 / 0 |

Flattening the opaque tag and address attributes changes how the tool groups
RDC findings. Expanding the reports to bit endpoints reproduced the previous
64,486 and 105,010 source/sink pairs for two and eight channels respectively.
Separate connectivity queries confirmed zero fanout at the affected unused
sinks. The standalone register mailbox CDC run reported zero findings. These
reviews are bound to the local dependency graph. A released dependency and
lockfile require a fresh identity check.

At RTL checkpoint `beb343fe` with the local `bus_cell` sources from
`debfcf6`, all four control-budget synthesis profiles met the 1 GHz core
and bus and 500 MHz register targets. The table compares final mapped
results with the corresponding pre-extraction profile and technology inputs.

| Profile | Logic area change | Vectorless power change | Worst setup slack |
| --- | ---: | ---: | ---: |
| 1 channel, 32-bit memory | +0.061% | -0.369% | 0 ps |
| 2 channels, 64-bit memory | -0.340% | -0.651% | 0 ps |
| 3 channels, 128-bit memory | +0.009% | +0.072% | 0 ps |
| 8 channels, 64-bit memory | +0.006% | +0.179% | 0 ps |

Each profile retained the same number of sequential cells as its baseline.
The restored mapped databases passed the memory qualification and tightened-
clock rejection checks. Power estimates use default activity. The RAM leaf
models boundary loading only. Physical SRAM and routed timing remain open.

## 2026-11-19 core timing follow-up

Each write-one action now takes its pulse directly from the decoded channel
register. The Copy window selects a forwarding candidate from the returned row
and gates its transfer with the existing stop/valid handshake. Queue depths,
clock periods and pipeline cuts are unchanged.

Both native backends passed all 27 tests. A differential simulation compared
the previous and revised channel register modules on identical inputs across
114,688 combinations of address, access width, write strobe and lifecycle
controls. All response, state and action outputs matched. The admin reset
still fired on valid writes while work-register writes were blocked.

| Channels | CDC raw / reviewed / unresolved | RDC raw / reviewed / unresolved |
| --- | --- | --- |
| 2 | 66 / 66 / 0 | 1805 / 1805 / 0 |
| 8 | 210 / 210 / 0 | 2819 / 2819 / 0 |

The new RDC reports remove 358 and 1,352 source-to-sink paths respectively.
Comparison of the native endpoint and reset fields found no added paths.
Fresh connectivity queries found zero endpoint fanout on each affected sink
and confirmed that the selected channel and owner registers still have consumers.
All seven source-bound review tests passed, including the stale-identity and
missing-sink rejections.

At 128-bit memory width, the original testbench could not generate a Copy
starting on lane 15. The random task and its alignment coverage now use the
configured beat width. The last-lane scenario checks the allocated addresses
and the copied bytes with the source and destination taking turns on that lane.
At 1024 bits it reaches lane 127. Error tests span the injected address after
the page boundary, and fault-replay tests change only the bytes retained by
the failed write. Delaying the Fence write address by the Fence64 beat count plus
two cycles exercises W-before-AW even with a payload capacity of 256 beats.

A separate page-aligned 2KiB Copy on the 64-bit port produced one 256-beat
read and one 256-beat write. Both directions sustained eight bytes per cycle
within that burst. The scoreboard observed the Fence and verified destination
bytes and guard bytes. This single-burst observation does not measure the gap
between consecutive 256-beat bursts.

The default eight-channel build passed 32 scenarios at eight seeds each. All
256 positive runs contributed to the merged coverage database. The PDMA scenario
group hit 18/18 bins, and eight independent checker mutations were rejected.
The separate synchronizer-delay build passed 16 scenarios and rejected eight
mutations. At the matched default two- and eight-channel settings, the 16KiB
zero-delay Copy retained 2,048 beats in 2,048 cycles per direction and 2,161
cycles from descriptor to Fence. The eight-channel timing probe measured 4.18ns
from core issue to endpoint and 4.82ns from return FIFO to core.

Across the default profiles for channel counts 1 through 8 and the narrow
one-channel and wide three-channel profiles, the final testbench passed 408
positive runs and rejected 80 checker mutations. This total includes the 256
eight-channel runs above. The four limit profiles below bring the total to
448 positive runs and 112 expected rejections across 14 configurations.

Four additional profiles each passed ten Copy, lane, register, error and
fault-recovery scenarios and rejected all eight checker mutations:

| Channels | Memory width | Maximum beats per burst | Slots per channel |
| ---: | ---: | ---: | ---: |
| 3 | 256 bits | 3 | 3 |
| 2 | 512 bits | 8 | 5 |
| 1 | 1024 bits | 1 | 1 |
| 1 | 64 bits | 256 | 1 |

All four used 32-bit registers and eight read and write credits. These are
functional results; they establish no PPA or sustained-throughput guarantee
for the four profiles.

The following control-budget synthesis runs use the RTL in `9500c28c`. The
core and bus targets are 1GHz and the register target is 500MHz. Each run
passed the mapped-cell policy and the instance-scoped crossing checks.

| Profile | Worst setup slack | Mapped instances | Sequential instances | Logic cell area | Min. crossing slack | Min. RAM boundary slack |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Narrow, 1 channel | 0ps | 20,938 | 4,603 | 6,850.068 | 262.4ps | 277.6ps |
| Default, 2 channels | 0ps | 70,627 | 23,571 | 29,500.674 | 220.3ps | 65.9ps |
| 128-bit memory, 3 channels | 0ps | 69,820 | 16,688 | 24,188.609 | 249.3ps | 193.4ps |
| Default, 8 channels | 0ps | 157,302 | 40,143 | 56,966.713 | 207.1ps | 101.2ps |

The three-channel core now meets setup at the same constraints that gave
−43.2ps at `4de9cb7f`. Only the channel-register and Copy-window modules
changed in the emitted RTL for that profile. Relative to the preceding
matched runs, logic area decreased 20.12% on the three-channel profile, 3.03%
on the default two-channel profile and 0.41% on the default eight-channel
profile. It increased 1.96% on the narrow one-channel profile. All four
results are pre-layout logic estimates. The RAM model excludes intrinsic
SRAM timing, area and power.

Reopening each of the four saved databases reproduced the reported worst
slack and passed the RAM-load, hierarchy, mapped-cell and crossing checks.
Tightening the core clock in each restored database produced −100ps worst
slack and the timing gate rejected all four cases.

## 2026-11-18 configurable ownership boundaries

This candidate separates `DmaControl`, `CopyEngine` and `DmaMemoryBridge` without
adding a pipeline cut. The [configuration contract](../../docs/rtl/configuration.md)
defines independent inputs and derived geometry. RTL, VIP/checkers and
implementation use the same immutable snapshot profile.

The frozen eight-channel RTL passes all **31 scenarios at eight seeds: 248 positive
runs**, plus all eight checker mutations with their expected diagnostics. Separate
synchronizer-latency stress passes 16 positive runs and rejects the same eight
mutation classes. The native suite passes 27 tests, including directional credits,
narrow/full bus lanes, one/non-binary slot counts and Fence64 on a 32-bit port.

Fresh source-bound CDC/RDC checks pass at both required channel counts:

| Channels | CDC raw / reviewed / unresolved | RDC raw / reviewed / unresolved |
| --- | --- | --- |
| 2 | 66 / 66 / 0 | 2163 / 2163 / 0 |
| 8 | 210 / 210 / 0 | 4171 / 4171 / 0 |

All seven native review-gate tests pass, including rejection of stale input
identity, incorrect/missing endpoints and invalid grouping. The new source
inventory is independent of checkout/run location while retaining native compiler
options, source contents, lockfiles and package-owned resources. Snapshot byte
checks remain separate; relocation is not permission to reuse different RTL.

The 248-run coverage merge preserves every input and adds no exclusions. The
declared PDMA scenario group covers 18/18 bins. Selected module line/condition/
branch coverage is:

| Module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| CopyEngine | 89.74% | 74.45% | 81.94% |
| CopyWindow | 97.73% | 83.06% | 97.44% |
| CopyReadEndpoint | 100% | 81.82% | 100% |
| CopyWriteEndpoint | 100% | 81.97% | 100% |

This finite scenario group is not exhaustive architecture coverage. Raw reports
retain VIP coverage, unexercised protocol-error paths and reserved bits; neither
the scenario score nor module line coverage claims full parameter-space coverage.

At the unchanged default 64-bit memory profile, aligned 16KiB Copy still transfers
2048 read beats and 2048 write beats in 2048 cycles per direction, with zero payload
bubbles. Descriptor-to-Fence latency remains 2161 bus cycles. The internal observer
also retains the previous 4.18ns issue-to-endpoint and 4.82ns receipt-to-core values;
the arbitration-to-issue register path remains two core cycles.

The final functional matrix passes **391 positive runs**, with eight checker
mutations rejected in each of its ten configurations:

| Profile | Positive runs | Scope |
| --- | ---: | --- |
| Default, 1 channel | 6 | Copy, random traffic, reset, ring wrap and registers |
| Default, 2 channels | 31 | Full scenario suite |
| Default, 3..7 channels, each | 12 | Count boundaries and independent peer lifecycle |
| Default, 8 channels | 248 | Full suite, eight seeds |
| 3 channels, 64-bit registers / 128-bit memory | 26 | Three slots, read/write credits 2/3, FIFO depths 4/4 |
| 1 channel, 32-bit memory | 20 | Three beats, one slot, read/write credits 1/2, FIFO depths 4/4 |

The first narrow-profile implementation preparation rejected FIFO depth two:
the pinned cell's constraint contract requires at least four. The resolver now
rejects that input before tool launch. The supported narrow profile was rebuilt
and all 20 scenarios plus eight negatives rerun; the earlier depth-two simulation
does not qualify a supported integration. Six resolver tests pass, and the native
implementation-profile checks reject 27 invalid configurations and six invalid
channel counts while accepting every count in 1..8. Widths 256/512/1024 and the
256-beat boundary have elaboration evidence only, not functional or PPA closure.

Genus control-budget synthesis of checkpoint `4de9cb7f` met setup for three configurations. Core/bus
remain at 1GHz, registers at 500MHz, with no ULVT and no relaxed crossing budgets:

| Profile | Worst setup slack | Mapped instances | Sequential instances | Logic cell area (library units) | Minimum crossing slack | Minimum RAM boundary slack |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Default, 2 channels | 0ps | 80,292 | 23,571 | 30,423.749 | 224.9ps | 75.6ps |
| Default, 8 channels | 0ps | 158,771 | 39,913 | 57,202.532 | 199.7ps | 102.2ps |
| Narrow, 1 channel | 0ps | 20,360 | 4,604 | 6,718.685 | 262.3ps | 223.2ps |

Each passes eight crossing groups, four held-payload checks and eight local
synchronizer chains; minimum local-chain slack is 820.6ps. At matched default
two-channel settings and technology inputs, logic area increases **2.01%** from
29,824.451. Throughput and command latency remain unchanged; this is not a claim
of cost-free parameterization. The narrow profile has different resources and is
not a like-for-like area improvement over that baseline.

At eight channels, logic area decreased **14.26%** from 66,717.573 under the
matched constraints. Both default-profile restored databases passed the RAM, load,
boundary and hierarchy audits and rejected the tightened core clock.

The three-channel profile with 64-bit registers and a 128-bit memory port missed
core setup by **43.2ps**. Its bus and register clock groups met setup, and all
mapped crossing checks passed. In 18 of the 20 worst paths the register request
decode fed the core-side memory-request FIFO. The other two paths ran from
event-ring capacity to channel fatal state.
Physical SRAM area, power and intrinsic timing remain outside
the control-budget model. Zero worst slack provides no extra timing margin.

## 2026-11-16 implemented microarchitecture and timing observations

The [microarchitecture](../../docs/rtl/microarchitecture.md) maps the current RTL's
owners, arbitration, FSMs, pipeline cuts and lifecycle timing. The separate
[performance report](../../docs/rtl/performance.md) defines and records payload,
command/batch and internal-boundary measurements.

Fresh 2/8-channel builds passed 16 runs in total: five timing scenarios and one
observer-disabled case per configuration, two aggregate multichannel runs, and
write-fault/multichannel-reset runs checking observer custody. The ten original
throughput observations exactly match the observer-free baseline. Production RTL,
constraints and instance hierarchy did not change. No synthesis or CDC/RDC rerun
is claimed for this documentation/observation change. Raw logs and private tool
configuration remain outside tracked content.


## 2026-11-16 multichannel closure

The normal two-channel mapped diagnostic reports roughly −344ps worst setup
slack before optimization. A separate register-to-register query also reports
−344ps from a command-ring consumer to request-FIFO address storage; this is not
solely a reset-input path. Full baseline optimization completed with −186.3ps
worst setup slack, failing the 1GHz target; library and crossing-budget checks
passed. The timing-cut version, before the later decode correction, subsequently
reached 0ps with all implementation gates passing. That is diagnostic evidence,
not qualification of the final source.

The execution pipeline registers the arbitration grant and retains each channel's current
control-request address before issue. Event publication has an explicit address
preparation phase. No clock, crossing budget, library policy or external protocol
was relaxed. Both native compiler backends pass 21/21 tests; the local 64-byte
Copy fixture retains its 10/12 bus-cycle results without/with backpressure. The
Fence resume fixture now waits within its existing request timeout instead of
assuming immediate arbitration, retaining every target/value/width/context check.

Decode review also reproduced malformed Fence acceptance: an invalid width
latched CommandRead fatal but still advanced the consumer. Acceptance and fatal
handling now share one validity predicate. Native tests also reject nonzero
Copy/Fence direct gaps and inactive union bytes before consumption, execution or
event publication. The unsupported-command scenario now constructs a valid Trap
record instead of changing the opcode on a Copy body.

Fresh two-channel regression passes 62 positive runs and rejects all eight checker
mutations. Long-copy read/write throughput remains 8 B/bus cycle with zero payload
bubbles. At matched settings, short/long descriptor-to-Fence latency increases
from 120/2160 to 121/2161 bus cycles; the extra admission register is visible at
command boundaries, not repeated for every payload burst.

Fresh source-bound gates accept CDC 66/66 and RDC 2163/2163, with zero unresolved
findings. The changed two-channel tag slices are checked against exact disabled
FIFO data-match pins: they have no endpoints and their flat traversal contains
only the queried input pin. The named unused arbitration intermediate has zero
fanout; live chosen/owner nets remain connected. The old source binding is rejected.
The final eight-channel regression passes all 248 positive runs and rejects eight
checker mutations. Its fresh CDC/RDC gates accept 210/210 and 4171/4171 findings,
with zero unresolved items; all 650 cited disabled FIFO inputs and RAM collision
nodes have no downstream endpoints. These remain path-specific reviews.
The full coverage merge preserves all 248 source runs: PDMA scenario bins 18/18,
top-level line 2596/2809 (92.42%), condition 1975/2983 (66.21%), branch 1240/1552
(79.90%) and toggle 87707/907796 (9.66%). No metric exclusions were added.

The retained matrix is complete: 31 positive runs at one channel, 62 at two,
18 each at three through seven, and 248 at eight: **431 positive runs**. Each
configuration rejects the eight distinct checker mutations; supplemental batches
repeat some negative controls and do not add new mutation classes. Eight-channel
synchronizer-latency stress adds 18 positive runs and eight rejected mutations.
All seven native review-gate checks pass on the final eight-channel source.

Fresh final-source control-budget mapping and restored-database audits pass at
both required implementation configurations:

| Configuration | Worst setup slack | Mapped instances | Sequential instances | Logic cell area (library units) | Minimum crossing slack | Minimum RAM boundary slack |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 channels | 0ps | 73,196 | 23,575 | 29,824.451 | 223.4ps | 49.4ps |
| 8 channels | 0ps | 175,196 | 40,147 | 66,717.573 | 191.8ps | 44.7ps |

Both use core/bus at 1GHz and registers at 500MHz, with no ULVT. Each mapped run
checks eight crossing groups, four held-payload paths and eight local synchronizer
chains; minimum local synchronizer slack is 820.6ps in both configurations.
Restoring each database passes the RAM/load/boundary and hierarchy audits;
tightening the core clock is correctly rejected. No clock, crossing budget or
reset constraint was weakened to obtain these results.

The instance totals include one opaque accounting RAM leaf; its physical area,
power and intrinsic timing are excluded. FIFO storage remains synthesized logic.
Zero worst slack provides no extra margin. These are pre-layout logic and boundary
budget results, not routed timing, real-SRAM, reset recovery/removal, synchronizer
MTBF or workload-power signoff. Earlier RTL results below retain their own snapshot.

## 2026-11-16 multichannel coverage and synthesis diagnostics

The full eight-channel regression (31 tests × eight seeds) now has an accepted
coverage merge for the pre-cut RTL, unchanged from the checkpoint below.
Direct aggregation fails in the installed tool even with short input paths;
bounded native merges preserve all **248 original simulation identities** and
leave every input database byte unchanged. A separate 31-run comparison produces
identical direct/staged scores and metric numerators/denominators. Paths containing
spaces also pass. Native merged-test labels count intermediate batches, not the
original simulations; the provenance receipt records the latter.

A negative test mixing a two-channel run into the eight-channel design exposed
zero-exit instance-shape warnings. The merge now rejects native warnings as well
as errors; strict tool flags alone do not guarantee complete metric preservation.

| Eight-channel ordinary regression | Covered / expected | Coverage |
| --- | ---: | ---: |
| Declared PDMA scenario bins | 18 / 18 | 100% |
| Top-level line | 2588 / 2799 | 92.46% |
| Top-level condition | 1951 / 2956 | 66.00% |
| Top-level branch | 1241 / 1552 | 79.96% |
| Top-level toggle | 87346 / 906760 | 9.63% |

No metric exclusions were added. Generic VIP coverage remains in the report and
is not a PDMA feature-coverage claim. The earlier “64/64” scenario-bin statements
misread the tool's `AUTO BIN MAX` columns; those reports show 100% for the declared
scenario group, not 64 covered bins. They are corrected below and in the linked
review records. This finite covergroup is not exhaustive architecture coverage.

Both previous full synthesis runs timed out during optimization after mapping;
neither is a PPA pass. The flow now saves a source/library-bound mapped database
and critical-path report before optimization. Native fixture tests restore that
database and verify that diagnostic mode cannot claim final qualification.
The two-channel timing investigation and final eight-channel implementation
qualification remain open. Physical SRAM cost/timing remain excluded.

## 2026-11-13 multichannel checkpoint

The shared Copy/Fence implementation supports every integer channel count in 1..8.
This checkpoint is ready for source review, but **implementation qualification is
not complete**: 2/8-channel mapping is still running, and the aggregate coverage
report has not been accepted. See [ownership and lifecycle](../../docs/rtl/multichannel.md).

| Channel count | Positive DUT runs | Rejected checker mutations |
| --- | ---: | ---: |
| 1 | 31 | 8 |
| 2 | 31 | 8 |
| 3, 4, 5, 6, 7 (each) | 18 | 8 |
| 8 | 248 (31 tests, eight seeds) | 8 |

The 400 positive runs use independent task/Fence oracles, channel/context memory
permissions and per-channel lifecycle assertions. Tests cover peer progress while
owners are parked, retained write data after source changes, independent reset,
IRQ routing and post-global-reset reconfiguration. Eight-channel synchronizer
latency stress adds 18 positive runs and eight rejected mutations; five TB selftests
also pass. Native tests pass 20/20 with both compiler backends. The model's 26
page-fault tests and strict DMA Clippy check pass, including the 1..8 retention
characterization; production model behavior was not changed.

Ordinary zero-delay throughput remains 8 B/bus cycle on both read and write data
channels, with zero payload bubbles in the short, long, batch, clock-ratio and
multichannel cases. This is simulation throughput for the selected 64-bit profile;
it is not a claim that the new maximum configuration meets physical timing.

Three review findings have failing-first evidence and fixes: a new global reset
must win over an old channel-clear acknowledgement at the same edge; global reset
must invalidate all software channel configuration state; channel reconfiguration
must not modify the device notification mask. Global register operations now have
a device-level software owner, while channels retain their rings and local reset.

Fresh source-bound gates pass at 2 channels (CDC 66, RDC 2151) and 8 channels
(CDC 210, RDC 4076), with zero unresolved findings. Seven native review-gate checks
accept the exact reviewed input and reject stale identities, wrong/missing endpoints,
missing coverage, invalid grouping and wrong counts. Raw reports remain available;
review counts alone do not establish safety. No physical CDC/MTBF/reset signoff is
claimed.

The installed coverage tool fails while merging the complete 248-run input set
(`URG-LTFF`), including with explicit 64-bit execution. The singled-out database
loads successfully by itself. The complete input set has not been reduced or
reported as a successful merge. Investigation of merge behavior, final mapped
results and remaining physical-budget checks are the next work items. SRAM
area/power and intrinsic timing remain excluded from control-budget synthesis.


## Reusable-cell boundary cleanup (2026-11-12)

Testbench manifest SHA-256:
`0bc3503373ff97828293b02efd259063022a491ef193fcaffe4d8f1242e91ded`.
Passive checker names now belong to `common_cell`; DMA still owns the bindings.
Library documentation and constraint examples no longer prescribe DMA usage.

- Mailbox and FIFO binding qualifications each pass their positive case and reject
  all four injected violations with the expected diagnostics.
- Focused random-clock regression passes aligned/long Copy, read/write PageFault,
  channel reset and DMA reset at seed 11, and rejects all eight checker mutations.
- All nine package-resource tests pass. The 95 generated hardware declarations
  match the previous checkpoint; executable package constraint Tcl is unchanged.
- Fresh native CDC/RDC field comparisons are identical to the previous checkpoint.
  The stale review is rejected before rebinding. Final checks report 40/40/0 CDC
  and 1363/1363/0 RDC raw/reviewed/unresolved findings, without changing review rules.

This is focused qualification of naming/documentation changes. The complete
regression and synthesis results below retain their original input identities;
no new synthesis or physical-signoff result is claimed by this cleanup.

## Ownership and package-flow cleanup (2026-11-12)

Final testbench manifest SHA-256:
`3b383f6a9906f525d3b0bb11aefbb2e8f008d96256dd1bd3944a1c14a1bdeacb`.
See the [organization review](../../docs/rtl/organization-review.md) for findings,
responsibilities and the failing-before smoke correction.

- Target and random clocks each pass all 24 DUT tests at seeds 11/17; the separately
  instrumented latency snapshot passes all 24 at seeds 23/31. Each final batch
  rejects all eight checker mutations: **144 positive runs, 294 scenarios,
  318 terminal checks and 219,925 checked bytes**.
- All 17 DMA and 10 common-cell native tests pass with CC and Cranelift. The five
  TB selftests pass at seeds 11/17. Resource transport has nine passing tests;
  profile validation rejects 16 invalid configurations. Tool-cache integrity,
  log failure detection, runner failure aggregation and process cleanup tests pass.
- All four generated outputs are current. Six generator CLI/oracle-preparation
  tests, C layout checking and the native RAL and Rust-codec/SV oracles pass.
- The retained standalone SV smoke failed first on the obsolete one-outstanding
  reset assertion. Its final Copy/Fence, drain and post-reset traffic checks pass.
  Mailbox and FIFO binding qualifications each pass their positive case and four
  corruption/custody negatives.
- Native CDC/RDC fields are identical to the previous reviewed checkpoint: 40 and
  1363 findings, all explicitly reviewed, zero unresolved. The old identity was
  rejected before rebinding; all seven review-policy tests pass. Static sensitivity
  tests and the current register-mailbox formal boundary also pass.
- Fresh mapped integration passes instance isolation, wrong-depth, missing-path,
  impossible-budget and masked-path checks. The full mapping passes the original
  frequency targets with **0ps worst setup slack**, no ULVT, 74,120 instances and
  302 references. Cell area remains 32,267.907 in library units; all 40 synchronizer
  FFs are preserved. Eight crossing groups, four payload checks and eight local
  chains pass, with minimum crossing/local slack 211.3ps/820.6ps. Tightening the
  restored mapping's core clock is correctly rejected.

Only the 48 ordinary random-clock positive runs enter the final coverage merge.
The declared scenario group remains at 100%; module line/condition/branch coverage and
the zero-delay results are identical to the burst-window checkpoint below. Long
aligned payload remains 8B/bus-cycle with no gaps and 2160 descriptor-to-Fence
cycles; short, batched and half-core results remain 120, 519 and 2200 cycles.
There are no new metric exclusions or weakened timeout/diagnostic gates.

This qualifies the refactored organization and flows for the same minimal profile.
It does not add physical routing, hold/reset recovery/removal, MTBF, workload-power
or full DMA feature signoff.

## Burst-window qualification (2026-11-12)

Current testbench manifest SHA-256:
`648510316c892f13eab824bca945e6ae3b9e2a4d5d4f9852af6f25cc9168cd34`.
The production RTL and implementation inputs are bound by the current
[`dma-copy-review.tcl`](../../impl/dma/reviews/copy.tcl).
The simulation-only synchronizer-latency build has a separate manifest and cannot
be used by the synthesis/static flows.

All 24 DUT tests pass at target clocks and randomized asynchronous clocks, each
with seeds 11 and 17. The same 24 tests pass with independently injected 0/1-cycle
synchronizer delay at seeds 23 and 31. Together: **144 positive runs, 294 scenarios,
318 terminal checks and 219,925 checked bytes**. Throughput cases explicitly use
their zero-delay clock profile, including a half-core variant, in every batch;
other scenarios use the selected target/random clocks.
All three final batches also reject all eight mutations with their intended fatal
diagnostics and finish successfully; no timeout or launcher failure counts as a
correct rejection.

The final error checker permits only zero or the whole first page fragment in
the read-error scenario; an ordered write error must report the exact prefix.
It checks reported bytes and does not require an unchanged suffix. Native window
tests control the error/receipt ordering and verify the exact frozen snapshot.
The eighth checker mutation reports an impossible partial-fragment prefix.

One intermediate regression aggregation was interrupted by editing its live
shell script. Its logs remain separate and it is not an aggregate pass. The
launcher now parses its complete body before execution; the current final runs
use the stable launcher. Runner fixtures verify parallel overlap, eight negative
diagnostics, timeout rejection and cleanup of resistant descendants.

Coverage below merges only the **48 ordinary random-profile positive runs**.
It includes no injected RTL, negative tests or new exclusions. The scenario
covergroup reaches 100% of its declared 64 bins; this is not exhaustive feature,
parameter or protocol-error coverage.

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 88.64% | 77.14% | 76.81% |
| CopyWindow | 97.20% | 81.95% | 95.16% |
| CopyMemoryEndpoint | 100.00% | 84.21% | 100.00% |
| CopyReadEndpoint | 100.00% | 80.70% | 100.00% |
| CopyWriteEndpoint | 100.00% | 83.12% | 100.00% |

Seventeen DMA native tests pass with both CC and Cranelift; ten common-cell tests
pass with both backends. These local tests are not merged into UVM coverage.
The W-before-AW fault case confirms actual data completion before its address
handshake and rejects premature Fence effects. Reset scenarios retain every
accepted write until its response, including multiple outstanding writes.
Performance, static review, constraints and mapping limits are recorded in the
[throughput review](../../docs/rtl/throughput-review.md).

## Testbench lifecycle cleanup (2026-11-11)

Testbench manifest SHA-256:
`9ec97793f8a1176f380b86821a6ac4aea012377a6417acd74063e7e6d42e4c5e`.
One fresh native build passes all **19 DUT tests x 8 random seeds** and all
**19 DUT tests with fixed clocks at seed 7**: **171 positive runs,
369 scenarios, 405 terminal checks and 32,045 checked bytes**.
Both batches reject all seven checker mutations with their intended diagnostics.
No positive DUT run reports a UVM error/fatal or assertion failure.

The ordinary DUT runs instantiate no clock-period selftest and log no
`CLOCK_CHECK` activity. Clock/reset-stimulus qualification is an explicit TB suite.
All **5 TB tests x 8 random seeds** pass; each also passes with fixed clocks in the
focused qualification. They test clock periods/reset edges, IRQ wait isolation,
timeout receiver cleanup, concurrent waits and reset-thread isolation. These
minimal fixtures do not exercise DMA task flows or contribute DUT coverage.

Before the fix, real native runs showed `IRQ_WAIT_SCOPE` after successful and
timed-out waits, and `RESET_WAIT_SCOPE` after reset: each operation killed an
unrelated caller-owned thread. Ordinary Copy and concurrent waits passed on that
same baseline. The corrected implementation preserves those threads and prevents
a timed-out receiver from consuming a later notification.

The parallel runner distinguishes DUT and TB suites, rejects unknown selection,
and excludes TB runs from DUT coverage. Runner tests also retain cancellation,
child-process teardown, timeout rejection and exact negative-diagnostic checks.
The coverage merge uses the build plus all 152 positive random DUT runs, excluding
negative and TB selftest databases. Native summary generation succeeds:

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

DUT RTL, AXI protocol checks and DUT invariant assertions are unchanged. This
qualifies the testbench lifecycle cleanup, not a new hardware feature, physical
signoff or measured wall-clock speedup. Raw logs and tool/deployment details stay
outside version control.


## UVM RAL qualification (2026-11-09)

Final testbench manifest SHA-256:
`2dcb7a6b8abf293c938cbf642d1a0ff07aa44a82bee53518ba0658851bfeb874`.
All **19 tests x 8 random seeds**, plus all **19 fixed-clock tests at seed 29**,
pass against this single fresh build: **171 positive runs, 369 scenarios,
405 terminal checks and 32,238 checked bytes**. Each batch also rejects all 7
checker mutations with the intended diagnostics. No positive run has a UVM
error/fatal or assertion failure. The external run watchdog is 300 seconds;
software polling and hardware reset/fault checks were not relaxed.

The new RAL scenario round-trips a 64-bit ring base through 32-bit AXI4-Lite, checks
that an unmapped read returns UVM_NOT_OK, restores configuration and completes
Copy/Fence. It first performs a real channel reset because disabling an already
configured channel does not unfreeze its ring configuration.

Separate real-UVM metadata tests cover 1/3 channels with 4/8-byte maps and 1020
channels across an aperture-alignment boundary, including relocation, fields,
access and reset metadata. A custom-schema oracle verifies valid zero parameters
and a source parameter named bus_bytes. Seven generator tests, one CLI test,
clippy, formatting and generated-output checks pass.

The initial RAL matrix exposed an existing directed write-fault fixture
assumption: one allocation allowed only 40 bytes before a page boundary, although
the test mutated all 64 source bytes. The final fixture aligns its 64-byte range
to 64 bytes so the full-source scratch-retention assertion is valid; no data
assertion was removed. General random/unaligned/page-crossing Copy tests remain.
The entire matrix was rerun after this correction.

Coverage summary uses all 152 positive random runs, with no metric/bin exclusions:

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

The scenario covergroup reaches 100% of its declared bins. Detailed report
generation crashes for this input set, including with 64-bit mode. Summary output
with the identical complete inputs succeeds. The flow uses text summaries and
retains raw databases; detailed report generation remains a tool limitation.
The executable tool selection was checked privately; no deployment paths are
included here. DUT RTL is unchanged, so this is register-model/verification
qualification, not a new synthesis or physical signoff result.

## Pre-RAL qualification (2026-11-09)

This checkpoint predates the generated UVM register-model migration. Its hardware
results remain useful history; it does not qualify the new register transport.

Testbench manifest SHA-256:
`6c43a423b3b7e68ac9cbe9d757740f6bb88e11eebed0adaae505b81283353547`.
One fresh build is shared by isolated test/seed runs; executable integrity is
checked before each launch. Raw logs and tool reports remain private.

- All **18 tests x 8 random seeds** pass: seeds 1, 7, 19, 41, 97, 211, 503, 997.
- All **18 tests** pass under the fixed-clock profile with seed **29**.
- Together these are **162 positive runs, 360 scenarios, 396 terminal checks**
  and **32,779 checked payload bytes**. No positive run has a UVM error/fatal
  or assertion failure.
- Both batches reject all **7 checker mutations** with their intended diagnostic.
  Early Fence publication through the IRQ error path and understated progress
  were incorrectly accepted before the checker fixes; both are now detected.
- The explicit event-boundary test checks a split error-event write, decoded
  identity/code/progress, exactly-once consumption and the untouched next slot.
- Runner fixtures cover parallel overlap, fixed-clock forwarding, input rejection,
  aggregate failure and child cleanup. A fatal message followed by a timeout is
  now a failed run, not an expected rejection; all real negative runs exited
  normally. Shell syntax, C11 layout checks and generated-record freshness pass.

The random-profile coverage merge includes all 144 positive runs, excluding
negative tests. The scenario covergroup reaches **100% of its declared bins**,
including actual allocation alignment and event-page crossing. This does not
mean exhaustive architecture coverage.

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

Combined HTML/text report generation crashed in the installed coverage tool.
The same complete input set succeeds with text output; the automated flow now
uses text reports without filtering metrics or adding exclusions.

The fixed profile has **33 successful Copy/Fence completions and 3,908 bytes**.
Submission-to-observation latency is **1.710 us minimum, 36.021 us weighted mean
and 103.160 us maximum**. This includes polling, injected delay and fault recovery;
it is not a throughput benchmark. Independent test randomization and allocation
placement differ from the earlier combined suite, so these cohorts do not form
a controlled performance comparison.

No DUT RTL changed in this cleanup. Native and synthesis evidence below remains
the earlier hardware baseline; those runs were not repeated. This qualification
is still limited to one-channel Linear Copy/Fence32/64, not other commands,
multichannel scheduling or physical CDC/RDC signoff.

## Independent-test checkpoint (2026-11-06)

All 17 independently selected tests have passing seed-1 results. The first
four-way run passed 16 tests and exposed the write-fault oracle's implicit
alignment assumption. That directed test mutates the source after a retained
burst read; its buffers now explicitly request bus-word alignment. All three
affected read/write/Fence fault tests were rebuilt and rerun successfully.
The manifests differ only in `tests/dut/fault.svh`:

- Initial manifest: `bcbb507ff7ac3c65a964c3241c304b921559b0cc8b43eb83c4f644aa660b6e6c`.
- Final manifest: `de2284e7d6c9c376b5aa61e835fe950629df045413472b3cf9b287bff698d75c`.

Five checker mutations are rejected with their required diagnostics. Runner
fixtures verify parallel overlap, aggregate failure, exact test markers, stale
output rejection, positive-only coverage inputs and child-process cleanup on
cancellation. Actual execution also caught and fixed the older Bash empty-array
behavior in the runner. Shell syntax, C11 layout checks and flow selftests pass.

The schema-backed SV generator has four passing Rust tests, reproducible output
and a simulator oracle comparing complete Copy/Fence byte images and Event
decoding against independent Rust codecs. The allocator dependency patch has
three failing-before scenarios and four passing-after scenarios; external
checkouts remain unchanged.

This is a pause checkpoint, not a new full coverage or performance qualification.
The complete multi-seed matrix and coverage merge have not been repeated after
the final ownership changes. RTL is unchanged from the native/synthesis baseline
described below.

## Earlier combined-suite baseline

The following measurements describe the earlier combined suite, before the
test split. They are retained for provenance rather than copied to the new runs.

Validated on 2026-11-06 after the testbench refactor and minimal Fence extension.
The sorted testbench input manifest has SHA-256
`47096cfb8cee0634e096a0214e858c286fbef71b8b135cc5ea0a12b990b5801f`.
RTL and testbench source manifests, complete logs, coverage databases and
implementation reports are retained with the local run artifacts.

## Observed results

- Random clock profiles: seeds **1, 7, 19, 41, 97, 211, 503, 997** pass.
- Fixed clock profile: seed **29** passes with `+clock_mode=fixed`.
- Each run completes 39 scenarios: **351 scenarios**, **387 terminal task checks**
  and **32,989 completed-task payload bytes** across all nine runs.
- All positive runs have zero UVM errors/fatals and no assertion failures.
- Five negative tests fail with the intended diagnostic: payload corruption
  (`TASK_DATA`), wrong and unknown identity (`TASK_IDENTITY`), omitted Fence
  (`TASK_TIMEOUT`), and duplicate terminal result (`TASK_DUPLICATE`).
- Native Veryl tests pass **13/13** with CC and **13/13** with Cranelift.
  New Fence tests cover scalar widths, lane guards, ordering behind a delayed
  Copy receipt, fault/resume, permanent write error, reset drain and invalid
  alignment/range. Failing-first Fence and word-strobe proofs were captured.
- The independent SV smoke, C11 contract-layout checks, shell flow selftest and
  mapped synthesis pass.

Successful flows poll a real Fence marker, explicitly check the task result and
memory, then drain reset before freeing their ring/payload resources. Software
reset remains in the scenario. Only power-on reset uses UVM `reset_phase`.
Three independently configured jarvisuk clock groups pass period and synchronous
reset-release checks. Handler and checker instances are channel-specific; the
single implemented channel uses platform vector 17.

## Coverage and performance

The scenario covergroup reaches **100% of its declared bins** for outcomes,
length classes, alignment, page-boundary cases and Fence width. The merged
coverage report includes the eight random-profile positive runs only, excluding
negative tests. This is not exhaustive architectural coverage.

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 92.55% | 77.54% | 84.62% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

Local native tests and this scenario suite have different purposes; native
coverage is not merged into these simulator percentages. Protocol-error paths,
unexercised guards and constant-width/profile bits remain visible in the report.
No exclusions were added to improve the score.

The fixed profile records 33 successful Copy/Fence completions and 3,576 bytes.
Submission-to-observation latency is **1.700 us minimum, 26.751 us mean and
100.360 us maximum**. It includes software polling, asynchronous clocks, injected
delays and recovery cases. These mixed small scenarios are a latency diagnostic,
not a peak-throughput benchmark. The passive subscriber reports byte counts and
latency without owning task resources or affecting stimulus.

This remains a one-channel Linear Copy/Fence32/64 RTL profile. The organization
supports channel-specific verification ownership, but does not establish
multichannel, other-opcode or analog CDC/RDC signoff. See
[requirements](requirements.md) for scenario mapping and
[dependencies](dependencies.md) for the platform/library boundaries.

## Future scope

Concurrent task/result routing and additional DUT features need their own
scenarios; the current software flow intentionally handles one task per channel
at a time. SV record and UVM register generation now use the published schema
backends; the older contract-local generator checkpoints above are historical.
