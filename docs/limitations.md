# Current limits

- Reviewed runtime profiles only; unreviewed runtimes remain unsupported.
  Current and earlier native platform qualification are distinguished in
  [validation](validation.md).
- Some historical Bank/epoch inputs may be unavailable. Alchemy questions remain
  open; recovery requires sufficient evidence, not guessed values.
- Benchmark: 47 unique targets, 41 strict matches and six disclosed CU/meter-log
  warnings. It does not prove universal coverage.
- The 47-case sweep is supplied/prepared replay with guarded hydration. Six
  separate cases reconstruct from captured sources.
- Broader fix43 signature testing verified 481 of 1,198 targets; 717 stopped as
  unsupported. Missing account images, transaction lookups and sysvars are the
  first observed blockers. This is not an estimate of all Solana transaction coverage.
- Collection without explicit limits requires a complete export or errors. Explicit
  limits permit TRUNCATED output, never a claim of complete collection. Safety ceilings
  remain. SBPF memory excludes native builtins and host-syscall bulk copies.
  Newest-worker invocation metadata has an explicitly weaker inventory check.
- Source debugging needs exact usable symbols. No custom IDE or true reverse execution.
- Large JSON/cache restoration remains memory-intensive. Synchronous provider
  calls obey their deadline but cannot always be interrupted immediately.
- Receipts bind evidence and implementation, not independent Solana consensus.
  The qualified host-identity model assumes statically linked coordinating binaries.
  Original-control matching does not guarantee every modified payload's behavior;
  backtesting does not predict inclusion or profit. `COMPLETED` means replay
  completed, not that the transaction succeeded. Inspect trace completeness metadata.
- Local offline bundles and Node/Bun SDK are implemented. Bundles must match the
  host OS/architecture and its [system dependencies](installation.md#platforms).
  Version 0.1.0 packages and platform qualification are complete.
  Demo inclusion was approved by the owner;
  no separate provider permission or endorsement is claimed.
  Ubuntu 24.04 and Debian 13 container qualification passed; this does not test
  independent hardware. No automatic worker download or latest-version substitution.
- Cleanup differs by OS: Linux uses a subreaper, macOS terminates the worker's
  process group, and Windows uses a kill-on-close Job Object. These are lifecycle
  controls, not equivalent security sandboxes. Windows forced termination cannot
  guarantee a final receipt; cooperative EOF cancellation can produce one.
- Timing depends on the host. Large trace journals can require several GiB of
  memory; a valid request can hit an explicit time/output limit on a slower host.
- The automatic signature path covers reviewed slots 353951234–451646918 and
  uses Alchemy. Profiles are compatibility strata, not unique validator identities.
  Other slots need additional reviewed runtime evidence, not extrapolation.
  The low-level builder still accepts explicit candidate/runtime input.
  Experimental debug remains available through Rust and hidden CLI JSONL; the SDK currently
  exposes simulation and trace requests, not a separate interactive UI.
- Surfpool support is our companion adapter, not an upstream feature or accepted PR.
  Anchor support builds strict transactions offline, not historical .rpc() calls.

[Validation and release status](validation.md).
