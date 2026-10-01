# Architecture

`src/engine/mod.rs` exposes the Rust API; `_6_workflow/` connects the stages.
Rust components telescope recursively: a small `mod.rs`, numbered children,
`_shared/` where needed, and `tests.rs` for a single component test file
or `tests/` for multiple files/helpers. Leaf components
may keep their implementation beside the index. Normal module declarations
follow the directory tree; aliases preserve existing public Rust API names.

Start at [src/cli/main.rs](../src/cli/main.rs) for the executable or
[src/engine/mod.rs](../src/engine/mod.rs) for the Rust API.

## Repository map

```text
src/                           Shipped implementation
  cli/
    main.rs                    Executable entry, CLI handling and worker-owner dispatch
    _0_arguments/              Commands and configuration
      _2_signature/            Signature CLI, JSON/@file inputs, wizard, output
        _2_inputs/             Parse collection, payload and override inputs
          _0_collect/          Map collection options to existing engine capture contracts
          _1_payload/          Parse JSON/@file payloads and account overrides
    _1_bundle/                 Pinned worker installation
    _2_debugger/               Experimental JSONL backend (no keyboard UI)
    _3_demo/                   Fresh execution of installed demo inputs
    _4_lifecycle/              Cancellation and child-process ownership
    _5_report/                 Opt-in, safe local internal-error issue draft
    _6_request/                Build requests from reviewed blocks/runtime bindings
    _7_sources/                Explicit data-provider selection
    _8_terminal/               Menus, progress and readable summaries
    _9_json/                   Terminal-aware JSON formatting; compact files and unchanged evidence
    _10_run/                   Execute CLI request and present its result
  engine/
    mod.rs                     Public API and stage exports
    _0_validate/               Request admission
    _1_resolve_runtime/         Pinned runtime selection
      mod.rs                   Stage entry and exports
      _0_catalog/              Validate installed catalog and worker identities
        mod.rs                 Component entry
        _0_implementation.rs      Catalog loading and validation
      _1_resolve/              Resolve requested and capture workers
      _2_registry/             Reviewed intervals, features and source pins
        _0_profiles/           Registry parsing, interval lookup and binding
        tests.rs               Registry tests
      tests.rs                 Catalog integration tests
    _2_prepare_state/          Accounts, dependencies and historical Bank
      _1_boundary/             Derive a unique transaction index from its block
      _4_source/               Fetch and reconstruct exact historical inputs
        _0_context/           Resolve Bank inputs and check reward consistency
        _1_hydrate/           Hydrate accounts; admit only proven sysvar recovery
        _10_credit_recovery/   Apply proof-bound, first-use System credit preloads
        _11_program_headers/  Recover program headers from bound historical evidence
      _5_controls/             Prepared boundary validation and original control
    _3_verify_original/        Historical original-control verification
      _3_archive/              Archived transaction evidence
      _4_state/                Archived account state
      _5_execution/            Execution comparison
      _8_verify_with_options/  Verification coordinator
    _4_simulate/               Original/variant execution, traces and debugging
    _5_finalize/               Results, typed failures and evidence receipts
      _3_job/_1_incomplete/    Failure categories and retained partial-evidence references
    _6_workflow/               Job coordination through the numbered stages
    _shared/                   Services reused by the numbered stages
      _0_bank/                Historical Bank, reward, sysvar and program proofs
        _8_slot_hashes/       Validate evidence-bound SlotHashes reconstruction
        _9_program_header/   Shared program-header proof contracts
      _4_dependencies/_8_credits/  Credit invariance, closure planning, proof validation
      _12_sources/_0_alchemy/  Bounded acquisition and finalized discovery cache
        _11_lookup/          Guarded transaction lookup recovery
  protocol/                    Exact JSON, request/result types and identities
  store/                       Persistent cache, blobs, leases and budgets
    _5_blobs/_1_windows.rs     Write-through, no-overwrite Windows blob publication
  bindings/typescript/         Thin Node/Bun client of the same Rust executable
    src/signature-input.ts     Private files for large SDK inputs
    src/signature-output.ts    Verify returned values against hashed artifacts
    src/adapters/
      index.ts                 Stable public exports only
      surfpool.ts              Historical companion wrapper
      anchor.ts                Serialize Anchor-built replacements
      ci.ts                    Assert replay completion
examples/                      Consumer recipes, one directory per use case
tests/                         Cross-component checks grouped by responsibility
docs/                          Public guides, capabilities inventory and qualification limits
```

One Cargo workspace. The TypeScript binding is a language binding, not another
engine. Root files are project manifests, policies and entry documentation.
rust-toolchain.toml pins local Rust/rustfmt to CI. SECURITY.md provides private
reporting instructions; .github/ owns CI and contribution templates.
There are no machine-specific data/cache directories in the source layout.
Each distributable includes LICENSE and NOTICE; packaging tests keep these
copies consistent with the repository's Apache-2.0 license for owned code.

## Execution

CLI / Rust caller / TypeScript binding → validate → resolve runtime → prepare
state → verify the original → simulate the request → finalize evidence.

Stages express ownership, not a single unconditional pass. Guarded discovery may
restart the complete original/requested attempt with additional exact inputs.
Interactive commands never retry automatically. Providers supply evidence;
workers execute pinned runtimes. Neither selects convenient substitutes.

A pinned runtime registry may declare `unsupportedIntervals` between reviewed
profiles. Each gap must match its declaration exactly; undeclared holes,
overlaps and unused declarations reject the registry. Lookup in a declared gap
returns `UNSUPPORTED_RUNTIME`, never a nearby executor. This allows independently
qualified eras to share a registry without claiming unreviewed coverage.

Signature `--collect` and SDK `collect` select additional observations; `--fields`
only selects returned fields. Both reuse the existing bounded capture engine and
`trace` output schema. The advanced prepared-request API keeps its explicit
capture/bounds contract; it does not accept the signature collection shorthand.

The signature mapper requires complete collection unless limits are explicit.
The shared `TraceOptions` completeness check runs before successful finalization;
incomplete artifacts retain their status even when the command fails. Runtime
memory hooks live in the separately pinned worker-source attachment, not the
TypeScript binding. Export bounds and capability checks stay in the Rust engine.

Component tests stay beside their owner: one file uses `tests.rs`; multiple files
use `tests/`. Tests are never numbered. Cross-component checks
stay in root `tests/`, without a numeric prefix. Rust child components are numbered
at every depth. Numbers express execution order for workflows and stable reading
order for collections of capabilities; they do not mean every child runs in sequence.
The TypeScript package, examples and public guides retain their native conventions.
The target is ~50 lines for module indexes and ~200 for other code files.
The layout check permits 70/250 lines so small overruns do not cause pointless
splits. One 253-line opt-in historical debugger scenario has a documented
260-line ceiling to keep its setup and assertions together.

Every numbered component has `mod.rs`. Routing levels contain no loose Rust
implementation files. At a leaf, `_0_implementation.rs` holds the existing logic;
short leaf implementations can fit directly in `mod.rs`. Public aliases keep
paths such as `svm_replay_engine::shared::runtime` stable. Production modules
never use `#[path]` redirects; those are reserved for explicit test ownership.

Native implementations stay with their owning component. `mod.rs` holds shared
contracts and platform selection; numbered Unix components share Linux/macOS
behavior, while Linux, macOS and Windows children hold genuine differences.
This applies to worker processes, debugger I/O, cancellation and worker ownership.
Do not duplicate Unix logic for symmetry. The layout check enforces these boundaries.

## Storage and installation

```text
<data-dir>/cache/index.sqlite   Index, leases and budgets
<data-dir>/cache/blobs/         Immutable observations and prepared states
<data-dir>/runs/job-*/          Requests, results, receipts and trace artifacts
```

Use OS application data by default; --data-dir chooses an explicit root.
Read-only installations are separate: bin/svm-replay, catalog.json, selected
workers and a hash-pinned bundle.json. Nothing writes into the installed package.
Old incompatible storage layouts reject rather than silently migrate.

Blob publication flushes file contents before committing the SQLite reference.
Unix also flushes the containing directory. Windows uses a write-through,
no-overwrite move instead of Unix directory fsync. Windows garbage collection
may leave unreferenced files after a crash; later collection removes them.
Missing or corrupted referenced blobs remain explicit errors, never cache misses.

## Mandatory layout check

```sh
node --test tests/interfaces/layout/repository.test.mjs
```

Run after every change. CI runs it too. It checks root ownership, example entry
points, routing-directory hierarchy, Cargo test registration, relative imports, documentation links and
machine-specific path leaks. Deliberate architecture changes update both this
guide and the test rules. A green layout check does not prove replay correctness.
