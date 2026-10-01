# Validation and release status

Version 0.1.0 validation evidence is recorded below. Qualification does not imply
acceptance by Surfpool/Anchor.

## Final platform qualification (1 October)

Platform qualification passed. The hosted suite includes
the complete Rust workspace, 48 prepared historical targets (the existing 47 plus
one early-runtime target), reference/capture parity for all seven runtime families,
seven SDK collection cases, installation and the offline demo.

| Platform | Current candidate result |
|---|---|
| Linux x86_64 | Local workspace and rebuilt installed-CLI checks passed; historical scope below |
| macOS ARM64 | 48/48 targets, seven capture/SDK families and installed demo passed |
| Linux ARM64 | Workspace, 48/48 targets, seven capture/SDK families and installed demo passed |
| Windows x86_64 / ARM64 | Workspace, 48/48 targets, seven capture/SDK families and installed demo passed on both |
| macOS x86_64 | 48/48 targets, seven capture/SDK families and installed demo passed |

The broader workspace checks exposed and fixed a Windows blob-publication bug:
Unix directory fsync was attempted on Windows. Windows now uses write-through,
no-overwrite publication. Additional fixes make synthetic fixtures, forced-kill
tests and loopback HTTP tests portable. Historical expected results were not changed.
The local rerun passed 472 Rust tests (22 existing ignored), 13 layout checks and
six installed CLI checks with networking disabled. Experimental interactive debugger
qualification is not included. Ignored tests and unsupported historical cases
are not counted as passes.

## Battle-test fix integration (1 October)

The main source tree now includes the fix43-qualified reconstruction, provider,
runtime selection and verification fixes. Experimental unknown-metadata recovery
is excluded. Local release-mode qualification is recorded below; earlier platform
passes do not automatically qualify these changes on other operating systems.

The frozen fix43 Linux CLI run tested 1,198 distinct historical targets:

| Result | Count |
|---|---:|
| Verified historical replay | 481 |
| Unsupported: unavailable account image with positive historical balance | 659 |
| Unsupported: transaction lookup unavailable | 56 |
| Unsupported: required sysvar unavailable | 2 |

All 469 previously passing fix21 cases remained passing, with 12 additional verified
replays and no timeouts. These are first observed blockers, not proof that supplying
one missing value would make every remaining case pass. This deliberately mixed
stress corpus is not a population-wide success-rate estimate. The 717 unsupported
outcomes are not passes. Runtime-worker sources remain separately hash-pinned.

Source integration checks and rebuilt-binary qualification are separate from this
retained campaign evidence. Current platform
qualification is summarized above. No new Alchemy capability or response is assumed.

The integrated tree passed 472 Rust tests on both Rust 1.94 (the project pin) and
Rust 1.98, with 22 existing opt-in/helper tests ignored. Local checks also passed
23 SDK tests, 57 cross-component tests, 13 layout checks, 13 packaging checks,
two consumer-example tests, pinned rustfmt and the npm package dry-run. These
are local source/interface checks, not a new full historical or platform sweep.

The rebuilt Linux review bundle passed `doctor` and four network-isolated real
signature CLI checks using copied caches: three verified replays (including two
fix43 gains) and one preserved explicit unsupported outcome. Original result
comparison, new coordinator identity and no-current-state-fallback assertions
passed. The subsequent complete cached rerun is recorded below.

### Final Linux release-mode rerun

All 1,198 frozen targets were attempted through the actual release CLI, using
copied caches, fresh receipts and a network-disabled namespace (79 minutes):

| Observed outcome | Count |
|---|---:|
| Verified historical replay, preserving every prior match | 481 |
| Same explicit unsupported outcome as the prior run | 661 |
| Not reproducible offline: negative signature lookup was not cached | 56 |

The last 56 attempts returned `SOURCE_TRANSPORT`, not successful replays. Their
previous live attempts returned `SOURCE_UNAVAILABLE` for unavailable historical
signature status. Discovery errors return before the cache write; this run did
not fetch or invent replacement responses. These are disclosed verification gaps,
not freshly reproduced unsupported outcomes. No prior match was lost and no
reproducible unsupported reason changed. No new Alchemy requests were made.

The same release binary passed installed CLI field selection, pretty saved JSON,
human output and explicitly bounded memory capture, with truncation disclosed.
The packed npm SDK ran the real prepared demo and passed an isolated strict
TypeScript consumer check. All four Rust crates were packaged and build-verified.
The final Linux archive passed the packed npm installer, installed `doctor` and
installed `--demo` without network access. Source/license inventories and archive
bytes were checked; these checks are not macOS, Windows or ARM qualification.

The demo was rebound to the qualified worker identity. Its transaction/account
results are unchanged: restoring only the old `runtimeBindingHash` reproduces
the exact previously approved output hash. The expected hash was refreshed only
for that proven identity change; no historical execution expectation was relaxed.

Fresh local checks again passed 472 Rust tests (22 existing ignored), 23 SDK tests,
57 cross-component checks, 13 layout checks, 13 packaging checks, two example
checks and pinned formatting. At that local checkpoint, cross-platform validation
had not started; subsequent qualification is recorded above. Publication remains
subject to approval, and staged artifact hashes are retained for review.

## Earlier isolated battle-test fixes (29 September)

This branch is a new, unpublished diagnostic candidate. The earlier platform
results below belong to the previous candidate, not to these changes. The frozen
100-signature Linux CLI campaign has 81 verified replays and 19 explicit unsupported
outcomes: 17 require unavailable historical account images and two require exact
SlotHashes. All 13 CLI option controls pass. These are latest attempts across
pinned diagnostic candidates, not a full rerun on one final binary; no new
cross-platform qualification is claimed. Unsupported is not a replay pass, and
this stress sample is not an estimate of support for all Solana transactions.

Regressions cover request/read budget alignment, native vote authority fee
dependencies, runtime-owned writable-account sanitization, old-runtime nonce
context, and inactive reward-phase evidence for Agave 4.0 stake accounts.
Original-control failures retain `unverified-control-output.json` for diagnosis;
that artifact is explicitly unverified and never makes a replay pass.

End-slot account checks account for the block's exact fee reward at Bank freeze,
which occurs after transaction execution. The reward amount and frozen balance
must agree, and the receipt records this adjustment. Non-fee rewards are not
silently subtracted. Under the existing CU-warning policy, a nested meter line
may split the total difference between entry budget and local consumption only
when both portions reconcile exactly; state, errors, fees and semantic logs
remain strictly checked.
Live block acquisition requests reward metadata; its discovery-cache identity
distinguishes the new request from older captures without rewards. Previously
captured request hashes remain valid evidence of their explicitly limited inputs.

Agave omits token-balance metadata when neither token program appears in the
transaction. An empty metadata array in that case is not evidence that a token
account has no amount. Verification reports `NOT_RECORDED_NO_TOKEN_PROGRAM` for
that target metadata category; lamports, account data and any reported token
amounts remain checked. It does not invent missing historical account images.

Implicit BPF loader accounts also come from the historical parent slot. Runtime
placeholder account names can have different byte lengths, incorrectly rejecting
transactions with tight loaded-account-data limits. The transaction's own limit
is preserved; no tolerance or current-state substitution is applied.

## Previous candidate qualification (27–28 September)

That candidate's source passed 372 Rust tests (22 explicit opt-in/helper tests ignored),
21 SDK tests, two consumer examples, layout/packaging checks and formatting.
All four Rust crates were packaged and verified without publication.

Ten fresh transactions across six runtime families also passed through the actual
Linux signature CLI, separately from the prepared historical corpus. Eight option
and error-handling checks accompanied those runs. This does not imply that every
historical signature has sufficient provider data for replay.

| Native target | Prepared historical replays | Final qualification |
|---|---|---|
| Linux x86_64 | 47/47 | Passed |
| Linux ARM64 | 47/47 | Passed |
| macOS 15 Intel | 47/47 | Passed |
| macOS 15 Apple Silicon | 47/47 | Passed |
| Windows Server 2025 x64 | 47/47 | Passed |
| Windows 11 ARM64 | 47/47 | Passed |

The corpus retains six declared CU/meter-log warning policies. Native checks cover
reference/capture parity, real memory observations, SDK, installation and offline
demo execution. Bounded trace exports remain explicitly labelled when truncated.

Final validation corrected macOS pseudo-terminal test pointer types and Windows
module visibility paths. Large captures retain the 180-second default but permit
explicit deadlines up to 900 seconds; see [capture budgets](tracing.md).
After that one-line ceiling change, five platforms received focused final-binary
trace/CLI tests, installation and fresh demo checks using unchanged qualified
workers. Their full corpus was not rerun after that change. Intel's continuation
verified identical coordinator bytes and all 47 prepared request identities
before reusing its preceding corpus results; capture/debug/SDK and installer/demo
checks then passed with explicitly longer test deadlines. Historical expected
outputs were never changed. This qualifies the listed targets, not every OS version.

The sections below retain earlier staged evidence. Neither those results nor the
platform table above qualify the newly integrated battle-test candidate.

## Earlier local interface qualification

The 27 September collection-correctness follow-up passed 366 Rust tests (22
existing opt-in/helper tests ignored), 21 SDK tests, 55 Bun checks, two example
tests, 13 packaging checks, 13 layout checks, rustfmt and the npm package dry-run.

The subsequent presentation-only `--pretty` update passed 369 Rust tests (22
ignored), 21 SDK tests and all 13 layout checks. Installed Linux checks covered
doctor, offline demo, compact/pretty replay equivalence and a complete DeFi capture
saved as pretty JSON. Its decoded trace matched the original hash-verified artifact.
Workers were unchanged; the 47-case corpus below was not rerun for formatting.

Automatic terminal indentation subsequently passed 371 Rust tests (22 ignored),
including real pseudo-terminal checks for pretty stdout, compact saved files and
`--json`/`--pretty` precedence. The installed Linux binary was also checked in a
terminal and with redirected output. This changes presentation, not replay logic.

The collection-correctness coordinator rerun passed all 47 prepared historical targets against
unchanged output hashes, plus memory-enabled captures on all six runtime families.
The latter deliberately used explicit small collection limits to test labelled
truncation. A separate six-family run also passed complete collection with no
user-specified truncation limits:

| Runtime | Memory events | Collection status |
|---|---:|---|
| v2-3 | 106,737 | COMPLETE |
| v3-0 | 57,750 | COMPLETE |
| v3-1 | 15,163 | COMPLETE |
| v4-0 | 4,684 | COMPLETE |
| v4-1 | 15,793 | COMPLETE |
| v4-2 | 21,197 | COMPLETE |

These are six selected prepared transactions, not every transaction in the archive.

The installed Linux CLI and TypeScript SDK both returned a complete DeFi capture:
309,851 register rows and 106,737 memory events, with original replay verification
passing. Wide integers remain exact decimal strings for JavaScript. The CLI also
passed demo, cold/warm replay, missing/malformed/wrong keys, replacement plus
account overrides, field selection, JSON/@file collection, explicit truncation
and saved output. The warm test made zero RPC calls. Billing/access/rate-limit
HTTP responses were tested offline; the invalid-key 401 was tested live.

Regressions cover complete-or-error finalization (including persisted receipts),
memory export starvation, wide integers, raw-envelope versus trace-size budgets,
actual guest loads/stores and faults, filtering, consistent newer-runtime read
snapshots, and snapshot-budget exhaustion.
The worker-source attachment retains updated file hashes, incremental patches,
build-input provenance and runnable regression tests. Original workers and
historical expectations were not overwritten.

Three cold acquisitions completed, including Jupiter with 138 predecessors;
[cost measurements and assumptions](providers.md#measured-cold-examples) are separate
from billing guarantees. The complex run used explicitly larger provider/time budgets.
This follow-up changes capture workers. Earlier platform results below do not
qualify these new binaries; cross-platform reruns and publication remain separate.

### Earlier interface checks

The collection interface follow-up passed 359 Rust tests (22 existing opt-in/helper
tests ignored), 21 SDK tests, 13 layout checks and rustfmt. Tests cover `all`,
JSON/@file, category dependencies, filters/limits, documented JSON, removed-flag
rejection and SDK forwarding. The adapter split preserves existing imports.
This is local offline qualification, not a new live historical or cross-platform
run. Installed previews and release artifacts still need refreshing.

The recursive numbered-layout follow-up passed 355 Rust tests (22 existing
opt-in/helper tests ignored), 53 Bun checks, 13 layout checks, 13 packaging checks
and rustfmt. Rust tests emitted no warnings. Component test targets and public
API aliases were preserved. No historical corpus or cross-platform rerun was
performed for this layout-only follow-up; release bundles still need rebuilding.

The 27 September routing-layout correction passed the offline Rust workspace
suite, 52 Bun checks, 12 layout checks, 13 packaging checks and rustfmt. Helpers
were relocated into owned child directories without changing replay logic or
public Rust paths. The layout guard now rejects loose Rust files in routing
directories. No historical corpus or cross-platform rerun was performed for this
source-layout correction; earlier artifact evidence does not qualify a new binary.

The 27 September interface follow-up passed 355 Rust tests (22 opt-in/helper tests
ignored), 19 SDK tests, 50 cross-component tests, 2 example tests and 23 combined
layout/packaging checks. A real cached signature replay with complete traces passed
through an explicitly pinned bundle without installation.json, using zero RPC calls.
Wrong-pin rejection and saved pre-execution errors passed too. These focused checks
do not relabel the following 47-case run as a rerun of the newer coordinator.

The 26 September Linux x86_64 coordinator rerun passed:

- 47/47 prepared historical targets against unchanged expected output hashes
  (404 seconds). Existing six CU/meter-log policies remain disclosed.
- 6/6 captured-source reconstructions, including the complex Jupiter case
  (156 seconds). These are separate from the prepared corpus.
- Live signature acquisition into an empty cache, then an identical warm replay
  with zero RPC calls (13 cold requests). Same-account replacement/override/trace
  also reused the cache without RPC.
- SDK inline replacement, 200 KB override, complete trace exports, selected fields,
  provider-budget enforcement and custom pinned registry loading.
- Actual historical, Surfpool and Anchor example entry points using signatures;
  the Anchor replacement changed the intended transfer to one lamport.
- Offline demo; 353 Rust tests (22 opt-in/helper tests ignored), 18 SDK tests,
  48 cross-component tests, 2 example tests, 13 packaging checks, 10 layout checks,
  formatting and npm package dry-run.

This qualification does not claim all signatures are supported. Missing historical
inputs still reject. Runtime workers and historical baselines were not changed.
No cross-platform jobs were run for these changes; final platform requalification,
artifact refresh and publication approval remain separate release gates.

## Qualified baseline

The 24 September native baseline passed:

| Platform | Historical replays | SDK / trace / debugger / installation |
|---|---|---|
| Linux x86_64 | 47/47 | Passed |
| Linux ARM64 | 47/47 | Passed |
| macOS 15 Intel | 47/47 | Passed |
| macOS 15 Apple Silicon | 47/47 | Passed |
| Windows Server 2025 x64 | 47/47 | Passed |
| Windows 11 ARM64 | 47/47 | Passed |

Each platform retains **41 strict and six declared CU/meter-log warning policies**,
six SDK/trace/debugger sets and three lifecycle checks. Truncated exports remain
labelled. WSL2 Ubuntu 24.04 x86_64 passed the same checks on an earlier candidate,
not a fresh rerun of every later binary.

The 47 targets are prepared/supplied-state replay with guarded hydration.
Six separate cases test source reconstruction. One live Alchemy boundary acquisition
passed; this is not a claim of live account reconstruction for all 47 targets.
The [benchmark index](../examples/historical/cases.json) lists identifiers and pins,
not executable datasets. No universal coverage or independent consensus proof is claimed.

## Release-cleanup scope

The signature CLI adds automatic acquisition and reviewed runtime-profile selection.
A previously unused mainnet SOL transfer was fetched from Alchemy into an empty
data directory with caching disabled, reconstructed, executed and matched its
historical status, balances, logs and 150 CU without metadata differences.
The combined replacement/account-override/call-trace path also passed locally.
Local offline checks: 337 Rust tests passed, 22 opt-in/helper tests ignored,
plus 10 repository-layout checks. Warm-cache reuse, missing-input errors,
key-file handling, field selection, file output and the no-key demo were tested.
These checks do not extend the earlier six-platform qualification: this workflow
must receive user review before cross-platform requalification or publication.

The final cleanup changes guides, test descriptions and demo installation-error
classification. It does not change SVM execution, runtime workers, original-control
verification or historical expected outputs. The new demo test first reproduced
the defect, then passed with the fix.

Earlier CLI changes added a prepared-request wizard, terminal summaries/progress
and local issue drafts. These reuse the same engine. Linux-focused checks cover
23 CLI tests, nine product tests and ten layout checks; PTY checks cover menu
selection/cancellation and separation of JSON stdout from progress stderr. Native
Windows/macOS UI checks and final package refresh remain pending. These UI checks
are not a new six-platform historical-corpus qualification.

Rebuilt coordinators require new artifact hashes. Their focused build/demo checks
are recorded separately in the release review; baseline corpus evidence must not
be relabelled as a full corpus rerun of those new binaries.

## Other retained evidence

- Ordinary baseline: 315 Rust tests passed (22 explicit opt-in/helper tests ignored),
  41 Bun tests, 13 packaging checks and 11 Node SDK checks.
- Linux prepared cohort: 409 seconds; maximum per-command peak RSS about 996 MiB,
  under a four-CPU/eight-GiB cap. This is a measurement, not a performance guarantee.
- Ubuntu 24.04/glibc 2.39 and Debian 13/glibc 2.41 clean-container checks passed.
  Containers share a host kernel; they are not independent hardware.
- A corrupted Intel worker copy was rejected; a fresh copy from the pinned
  original passed. The failed copy is retained, not rebaselined.
- Windows ARM's earlier CIM timeout and PID-only cleanup failure remain recorded.
  The final harness checks PID plus creation time; it passed with original workers
  absent. The old failure alone cannot distinguish PID reuse from a process leak.

## Public tests versus release qualification

[Public CI](../.github/workflows/ci.yml) runs ordinary offline Rust checks and
Linux/macOS/Windows SDK checks. It does not silently fetch private historical
fixtures or run the six-platform native corpus on every pull request.

Release-native builds and historical qualification currently use a private
maintainer harness because retained provider datasets are not public.
[Tests](../tests/README.md) lists the reusable drivers and their explicit inputs.
[Clean-host checks](installation.md#clean-environment-installation-check) can run
without the development checkout. No external-user adoption is inferred from these checks.

## Release checklist

1. Freeze the candidate source. Run layout, formatting, affected tests and package
   checks from [the test guide](../tests/README.md).
2. Build the coordinator with the pinned Rust toolchain for each supported target.
   Reuse unchanged, hash-verified workers; rebuild/qualify workers only when their
   inputs change. Native archives retain their runtime source/patch/license packet.
3. Test the changed paths on affected platforms. For release-wide execution changes,
   run the full historical qualification. Never change expected outputs to pass.
4. Stage `npm pack` and `cargo package --locked` outputs, native archives, source
   archive, manifest and SHA256SUMS. Verify archive contents, licenses and hashes.
5. Retain the demo-data decision: owner approved inclusion; no separate provider
   permission is claimed. Candidate archives contain the demo; the Git repository
   and benchmark index do not contain provider snapshots.
6. Review exact artifact pins and approve publication. Publish protocol/store
   crates before engine, then CLI; publish npm and the matching GitHub tag/assets.
   Update release-status wording with the publication, not before availability.
7. Require `offline-checks` and all three portable SDK checks in branch rules.
   These settings are configured on GitHub, not enforced merely by committing YAML.

No package upload, public repository change or tag is authorized by running tests.
