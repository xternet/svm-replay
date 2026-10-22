# Current limits

- Pinned runtimes only. Newer slots try the newest pinned runtime with an
  explicit unqualified warning, not a claim of historical runtime equivalence.
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
- Bundles and state overrides: at most 16 transactions and 64 overridden accounts. The
  historical archive cannot prove that an account did not exist, so an account your transactions
  create needs `{"pubkey":"…","lamports":"0"}` (or a whole-account override). Address lookup tables
  a bundle creates cannot be used by later transactions of the same bundle, and a bundle after
  the target cannot use a lookup table the target changes (both are explicit errors). The runtime's feature
  set stays historical even when feature accounts are overridden. `--collect` and debugging do not
  support bundles or state overrides yet.
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
  Version 0.1.0 packages and platform qualification are complete; 0.2.0 is prepared and unpublished.
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
  Slots above that range try the newest pinned runtime with last-known features;
  receipts report `UNQUALIFIED_NEWER_SLOT`. Original-result verification remains
  required, and modified executions are not historically qualified. Earlier
  slots and declared coverage gaps still require reviewed runtime evidence.
  The low-level builder still accepts explicit candidate/runtime input.
  Experimental debug remains available through Rust and hidden CLI JSONL; the SDK currently
  exposes simulation and trace requests, not a separate interactive UI.
- Full historical developer-node integration requires matching integration builds
  of Surfpool/Anchor plus an installed SVM Replay bundle. Surfpool owns public
  RPC/WS, chain history and Studio; execution and historical account data stay
  in SVM Replay. An ordinary Surfpool using a historical datasource still has
  its own current VM and is not equivalent.
- Global historical `getProgramAccounts`, `getLargestAccounts` and token
  delegate scans need indexes Alchemy does not supply. A loaded-account subset
  must not be presented as a complete historical answer. Provider-backed owner,
  holder and address-history queries can require pagination and additional
  credits; cached/synthetic contract tests are not live provider qualification.
- Alchemy can omit required historical accounts, including vote accounts and
  some Bank/sysvar inputs. An uncached offline image is not proof that the
  provider does not have it. Missing, malformed or inconsistent inputs produce
  typed source errors; no current-tip fallback or invented empty image.
- Historical time is frozen by default. Advancing changes local state without
  replaying later archived transactions. Cross-epoch time travel is an explicit
  synthetic warp: the clock moves, but rewards, stakes, runtime and feature
  activations are not replayed.
- Historical programs must use an SBF format the selected runtime supports.
  Fast ELF fixture installation is not a loader deployment. Actual loader
  deploy/upgrade needs an explicit next-slot advance before the program is
  callable. A newer Solana CLI's feature pre-check can request additional
  historical images; `--skip-feature-verify` skips that client pre-check only,
  never native VM validation.
- Stateful historical command sessions require Unix sockets. Native Windows
  returns `UNSUPPORTED_PLATFORM`; use the Linux implementation in WSL2. Explicit
  `--parent-pid` lifetime binding is Linux-only and is refused elsewhere.
- The standalone `svm-replay fork` server and SDK `forkAt()` have a smaller
  RPC/WS surface than Surfpool: they do not supply Studio, chain-wide scans or
  ordinary Anchor account/log subscriptions. Their VM branch snapshots are
  not Surfpool chain-store snapshots. Do not advertise that route as full
  Anchor compatibility.
- Anchor's generated helper owns a Surfpool child per fork, not an endpoint
  shared by every test. The still-running Anchor command reaps children after
  the test process exits. SIGKILL of Anchor itself needs job/cgroup cleanup;
  finally blocks cannot run after SIGKILL. Template/platform qualification
  must be reported separately; Linux testing does not qualify another OS.
- Existing `startHistorical()` and account-datasource fixtures are legacy
  interfaces, not the full historical Surfpool execution path.

[Validation and release status](validation.md).
