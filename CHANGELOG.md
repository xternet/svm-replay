# Changelog

## 0.2.0

- Added historical forks through `svm-replay fork --after SIGNATURE|SLOT` and
  the SDK's `forkAt()`. Slot numbers and transaction signatures are detected
  automatically; legacy `slot:`/`tx:` prefixes remain accepted. Fork clocks stay
  frozen until explicit advancement. Local accounts, programs and transactions
  use the selected pinned runtime worker.
- Added transaction bundles and account/program overrides. Original replay must
  verify before modified execution. Each transaction has its own result;
  capability and size limits remain explicit.
- Historical session transaction-v1 execution requires the installed worker's
  `native-transaction-v1/v1` capability (currently Agave4.2). Older workers return
  `UNSUPPORTED_TRANSACTION_VERSION`; legacy/v0 execution is unchanged.
- Added named provider configuration in `svm-replay.toml`, `Surfpool.toml` and
  Anchor's `[historical]` configuration. Credentials can come from environment
  variables or files. Missing historical images produce typed source errors;
  an existing account is never silently treated as empty or fetched from tip.
- Added the private historical backend used by the companion Surfpool changes.
  `surfpool start --after SIGNATURE|SLOT` keeps Surfpool's public RPC, chain/store
  policy, SDKs, subscriptions and UI while SVM Replay supplies historical data
  and execution. Optional datasource matching is excluded from this integration.
- Added companion Anchor test configuration and Node/Rust templates with
  isolated per-test Surfpool forks and explicit providers. Test code chooses
  the boundary, funds local accounts, installs its program and advances the
  frozen clock. Historical configuration does not redirect `anchor localnet`
  or every Anchor command. These integrations require compatible companion tool
  releases; the examples do not imply upstream acceptance.
- Added clock/epoch control, reset, snapshots, local account/program edits and
  persistence contracts behind the historical backend. Standalone fork RPC is
  a smaller surface than Surfpool. Synthetic epoch travel does not replay all
  skipped blocks, settlement or rewards.
- Runtime output uses the pinned Agave release names while family IDs remain
  machine identifiers. Runtime provenance is not proof of the exact binary a
  historical validator ran. Newer slots carry an explicit qualification warning;
  see [runtime coverage](docs/runtimes.md) and [limitations](docs/limitations.md).
- Packages include seven execution and seven capture workers with their source,
  locks and build evidence. Replay, capture and debugger capability requirements
  are checked separately. A native bundle is required in addition to the npm
  launcher/SDK or Cargo-built coordinator.
- Added versioned progress events, readable terminal results, typed provider
  diagnostics and owned process/cancellation handling. Stdout remains the result
  channel for scripted callers.
- Added normal and historical Anchor CI recipes and opt-in offline integration
  jobs. Provider-recorded fixtures need separate publication clearance; native
  platform qualification and clean installation results are recorded separately.
- Fixed npm-bin installation so `svm-replay-install` runs through npm's symlink.

## 0.1.0 — historical replay fixes

- Added reviewed runtime profiles, historical transaction decoding and provider
  lookup recovery without current-state fallback.
- Added guarded credit/program-header/sysvar reconstruction and reward consistency
  checks; missing unprovable state still returns an explicit unsupported outcome.
- Corrected dependency, fee and original-result verification edge cases with owned
  regression tests. Non-completed jobs retain labelled partial diagnostic evidence.
- Integrated fix43 source: its frozen Linux campaign verified 481 of 1,198 targets,
  with 717 unsupported. The final Linux release CLI preserved all 481 matches:
  661 unsupported outcomes were reproduced and 56 uncached negative lookups
  could not be repeated offline. Final platform qualification is documented separately.
- Excluded unfinished unknown-metadata recovery experiments from the release tree.

- Collection without explicit limits now requires complete exports; safety ceilings
  produce errors, not successful truncated traces. Explicit limits allow partial output.
- Wide trace integers use exact decimal strings for JavaScript consumers. Export
  size limits no longer incorrectly cap the larger raw worker envelope.
- Added qualified Linux SBPF memory capture across six runtime families, fair
  register/memory export, and correct truncation for snapshot-budget exhaustion.
- Provider failures include safe key/access/billing/rate-limit guidance; receipts
  count attempted RPC methods. Explicit acquisition deadlines allow up to 900 seconds.

## 0.1.0 — CLI improvements

- Component tests use unnumbered `tests.rs` for one file or `tests/` for multiple files;
  production modules retain
  their numbered architecture. Cargo targets and checks use the updated paths.

- Terminal JSON is indented automatically; pipes and saved files stay compact.
  `--pretty` forces indentation everywhere, `--json` forces compact terminal output.
  Evidence hashes and debugger JSONL are unchanged.

- Signature collection uses `--collect all`, JSON or `@file`; the SDK uses
  `collect`. This replaces signature `--trace`/`--trace-options` and SDK `trace`.
  Categories, dependencies, filters and limits map to existing capture contracts;
  output remains under `trace`. The advanced prepared-request API is unchanged.
- TypeScript adapters have separate Surfpool, Anchor and CI modules; their
  public import path remains `@xternet/svm-replay/adapters`.
- Signature SDK calls honor the explicitly selected bundle/pin without requiring
  installation.json. `--out` saves typed pre-execution failures as well as results.
- Finalized signature/block discovery caching; fully warm signature replay can
  avoid RPC calls without bypassing provenance or corruption checks.
- SDK large inputs use private files; returned result/trace data is verified
  against pinned artifacts. Provider configuration applies to signature calls.
- Historical, Surfpool and Anchor recipes accept signature-based configuration.
- Signature trace configuration, platform-mode defaults, readable call trees and
  field-aware output; excluded trace payloads are not materialized.
- Signature SDK and Surfpool/Anchor companion entry points, automatic installed
  diagnostics and opt-in npm command activation.
- Signature acquisition/execution budgets, early cooperative cancellation, pinned
  captured Bank inputs and explicitly reviewed runtime registry overrides.
- Terminal home menu and prepared-request wizard with confirmation and cancellation.
- Offline demo selection, timed progress, readable results and account changes.
- Explicit `--json` / `--human` modes; scripted output remains machine-readable.
- Opt-in `--bug-report` local internal-error drafts; no automatic upload.
- One CLI entry file, with presentation and reporting in named component folders.

## 0.1.0 — Unreleased

- Release-candidate guides distinguish the packaged demo, historical reconstruction
  and advanced consumer recipes; publication is still pending approval.
- Demo startup reports invalid/unreadable installation metadata separately from a
  missing installation, with regression coverage.

- Signature-based request construction from an archived block or bounded Alchemy
  acquisition, with saved provenance. A reviewed slot-specific runtime binding
  remains required; no runtime-era guessing.
- Lightweight Linux/macOS/Windows SDK checks on pull requests.
- Historical preparation, original-control verification and modified-transaction
  simulation through explicitly selected, hash-pinned runtime workers.
- Account and program overrides, guarded dependency discovery, typed failures,
  persistent caching and evidence receipts without current-state fallback.
- Bounded trace exports; completeness is reported separately from replay correctness.
  The keyboard debugger is excluded. The retained JSONL backend is experimental.
- Rust API, CLI, TypeScript/JavaScript binding and Surfpool/Anchor/CI companion
  examples. These do not imply upstream acceptance or a native historical Surfpool Bank.
- Offline bundle installation with integrity checks, cancellation and durable
  artifacts. Native Linux, macOS and Windows transports; see the platform
  qualification matrix for completed tests and prerequisites.
- Owned packages include Apache-2.0 and NOTICE. CLI packaging excludes development
  dependencies; engine test helpers remain inside their crate.

Owned code is licensed under Apache-2.0. Runtime sources include their patch and
dependency notices; provider snapshots require a separate publication basis.
The Ubuntu 24.04 and Debian 13 container runs above belong to prior qualification,
not proof for every new release artifact. See [validation](docs/validation.md)
and [limitations](docs/limitations.md) for the scope of current results.
